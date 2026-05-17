use adw::{prelude::*, subclass::prelude::*};
use core::cell::{Cell, OnceCell, RefCell};
use core::sync::atomic::Ordering;
use gtk::CompositeTemplate;
use gtk::{gdk, gio, glib};
use std::rc::Rc;
use std::sync::Arc;
use std::time::{Duration, Instant};

use crate::UI_TIMEOUT;
use crate::excuses::{EXP_INIT, EXP_RX};
use crate::library::{Albums, ToQueue, ToShuffledQueue};
use crate::player::{PlayerRequest, player_tx};
use crate::ui::{AlbumObject, AlbumOrdering, ItemTile, SortConfig};
use crate::ui::{UpdateUI, fallback_album_image, ui_tx};
use crate::util::search;

#[derive(Default, CompositeTemplate)]
#[template(file = "albums_page.ui")]
pub struct AlbumsPage {
    #[template_child]
    play_button: TemplateChild<adw::SplitButton>,
    #[template_child]
    sort_button: TemplateChild<adw::SplitButton>,

    #[template_child]
    view_stack: TemplateChild<adw::ViewStack>,
    #[template_child]
    albums_grid: TemplateChild<gtk::GridView>,

    #[template_child]
    search_entry: TemplateChild<gtk::SearchEntry>,
    search_query: Rc<RefCell<String>>,

    albums: RefCell<Vec<AlbumObject>>,
    filter: Rc<RefCell<gtk::CustomFilter>>,
    sorter: Rc<RefCell<gtk::CustomSorter>>,

    sort_mode: OnceCell<SortConfig<AlbumOrdering>>,

    shuffle: Cell<bool>,
}

#[gtk::template_callbacks]
impl AlbumsPage {
    #[inline]
    pub fn init_search(&self) {
        let filter = Rc::clone(&self.filter);
        let sorter = Rc::clone(&self.sorter);
        let search_query = Rc::clone(&self.search_query);
        self.search_entry.connect_search_changed(move |entry| {
            search_query.replace(entry.text().to_string());
            filter.borrow().changed(gtk::FilterChange::Different);
            sorter.borrow().changed(gtk::SorterChange::Different);
        });
        // TODO: Empty the query when pressing escape
        // TODO: Focus the search bar with CTRL+F
    }

    #[template_callback]
    pub fn handle_play_now(&self) {
        let model = self.albums_grid.model().expect(EXP_INIT);
        let n_items = model.n_items();
        let mut albums = Vec::with_capacity(n_items as usize);

        for i in 0..n_items {
            albums.push(Arc::clone(
                (model.item(i).unwrap().downcast_ref::<AlbumObject>())
                    .unwrap()
                    .shared_album(),
            ));
        }

        let player_tx = player_tx();
        player_tx
            .send(PlayerRequest::LoadQueue {
                queue: match self.shuffle.get() {
                    true => albums.to_shuffled_queue(),
                    false => albums.to_queue(),
                },
                shuffled: None,
                track: 0,
            })
            .expect(EXP_RX);
        let _ = player_tx.send(PlayerRequest::TogglePlay(Some(true)));
        let ui_tx = ui_tx();
        ui_tx.send(UpdateUI::OpenSheet(false)).expect(EXP_RX);
        ui_tx.send(UpdateUI::FocusPlaying).expect(EXP_RX);
    }

    #[inline]
    pub fn set_shuffle(&self, shuffle: bool) {
        self.shuffle.set(shuffle);
        self.play_button.set_icon_name(match shuffle {
            false => "media-playback-start-symbolic",
            true => "media-playlist-shuffle-symbolic",
        });
    }
    #[inline]
    #[must_use]
    pub const fn get_shuffle(&self) -> bool {
        self.shuffle.get()
    }

    #[inline]
    pub async fn load_albums(&self, albums: &Albums) {
        if albums.is_empty() {
            self.albums_grid.set_model(None::<&gtk::NoSelection>);
            self.view_stack.set_visible_child_name("empty");
            return;
        }
        self.view_stack.set_visible_child_name("albums");

        // The timers are used to reduce major UI stutters
        // by turning them into multiple smaller ones
        let wait = Duration::from_millis(10);
        let mut async_timer = Instant::now();

        let mut album_objects = Vec::with_capacity(albums.len());
        for (index, album) in albums.iter().enumerate() {
            // NOTE: Scope is required due to a Clippy warning false-positive
            // when `MutexGuard`s are explicitly dropped before the `await` point
            // Issue link: <https://github.com/rust-lang/rust-clippy/issues/6446>
            {
                let shared_album = Arc::clone(album);
                let album_locked = album.lock().unwrap();
                album_objects.push(AlbumObject::new(
                    index as u32,
                    album_locked.title(),
                    album_locked.artist().lock().unwrap().name(),
                    album_locked.year() as u32,
                    shared_album,
                ));
            }

            if async_timer.elapsed() > UI_TIMEOUT {
                glib::timeout_future(wait).await;
                async_timer = Instant::now();
            }
        }
        let model = gio::ListStore::new::<AlbumObject>();
        model.extend_from_slice(&album_objects);
        self.update_sort_fields(&model).await;
        self.albums.replace(album_objects);
        glib::timeout_future(wait).await;

        let query = Rc::clone(&self.search_query);
        let filter = gtk::CustomFilter::new(move |object| {
            let album_object = object.downcast_ref::<AlbumObject>().unwrap();
            let query = &query.borrow().to_lowercase();
            let score = search::query_score(query, &album_object.album().to_lowercase())
                .max(search::query_score(query, &album_object.artist().to_lowercase()) / 4.0);
            album_object.set_rank(score);
            score > 0.01
        });
        let filter_model = gtk::FilterListModel::new(Some(model), Some(filter.clone()));
        self.filter.replace(filter);
        glib::timeout_future(wait).await;

        let sort_mode = *self.sort_mode.get().unwrap();
        let sorter = gtk::CustomSorter::new(move |object_a, object_b| {
            let album_a = object_a.downcast_ref::<AlbumObject>().unwrap();
            let album_b = object_b.downcast_ref::<AlbumObject>().unwrap();
            album_a.order_cmp(album_b, sort_mode)
        });
        let sort_model = gtk::SortListModel::new(Some(filter_model), Some(sorter.clone()));
        self.sorter.replace(sorter);
        glib::timeout_future(wait).await;

        self.albums_grid
            .set_model(Some(&gtk::NoSelection::new(Some(sort_model))));
    }

    #[inline]
    pub async fn update_sort_fields<M>(&self, model: &M)
    where
        M: IsA<gio::ListModel> + ListModelExt,
    {
        // The timers are used to reduce major UI stutters
        // by turning them into multiple smaller ones
        let wait = Duration::from_millis(10);
        let mut async_timer = Instant::now();

        let mut i = 0;

        while let Some(item) = model.item(i) {
            // NOTE: Scope is required due to a Clippy warning false-positive
            // when `MutexGuard`s are explicitly dropped before the `await` point
            // Issue link: <https://github.com/rust-lang/rust-clippy/issues/6446>
            {
                let album = item.downcast_ref::<AlbumObject>().unwrap();
                let shared_album = album.shared_album();
                let album_locked = shared_album.lock().unwrap();

                album.set_rating(album_locked.sort_rating(3.0));
                album.set_played(album_locked.average_play_count());

                let song = album_locked.first_song();
                let info = song.info();
                let info = info.user();

                album.set_modified(info.modified);
                album.set_added(info.added);
            }
            drop(item);

            if async_timer.elapsed() > UI_TIMEOUT {
                glib::timeout_future(wait).await;
                async_timer = Instant::now();
            }

            i += 1;
        }
    }

    #[inline]
    pub fn assign_artwork(&self, index: usize, artwork: Option<&gdk::Texture>) {
        let albums = self.albums.borrow();
        if index < albums.len() {
            albums[index].set_property("artwork", artwork);
        }
    }

    #[template_callback]
    pub fn handle_reverse_sort(&self) {
        let reversed = self.sort_mode.get().expect(EXP_INIT).reversed;
        let reverse = !reversed.get();
        reversed.set(reverse);
        self.sorter.borrow().changed(gtk::SorterChange::Inverted);
        self.sort_button.set_icon_name(match reverse {
            true => "view-sort-ascending-symbolic",
            false => "view-sort-descending-symbolic",
        });
    }
    #[inline]
    pub async fn set_sort_mode(&self, sort_mode: AlbumOrdering) {
        let ordering = self.sort_mode.get().expect(EXP_INIT).ordering;
        ordering.replace(sort_mode);
        self.sorter.borrow().changed(gtk::SorterChange::Different);
        if let Some(model) = &self.albums_grid.model() {
            self.update_sort_fields(model).await;
        }
    }
    #[inline]
    #[must_use]
    pub fn get_sort_mode(&self) -> &SortConfig<AlbumOrdering> {
        self.sort_mode.get().expect(EXP_INIT)
    }

    pub fn uninit(&self) {
        for album in self.albums.take() {
            album.imp().is_visible.store(false, Ordering::Relaxed);
        }
    }
}

#[glib::object_subclass]
impl ObjectSubclass for AlbumsPage {
    const NAME: &str = "MellowAlbumsPage";
    type Type = super::AlbumsPage;
    type ParentType = adw::NavigationPage;

    fn class_init(class: &mut Self::Class) {
        class.bind_template();
        class.bind_template_callbacks();
    }

    fn instance_init(obj: &glib::subclass::InitializingObject<Self>) {
        obj.init_template();
    }
}
impl ObjectImpl for AlbumsPage {
    fn constructed(&self) {
        let _ = self
            .sort_mode
            .set(SortConfig::new(AlbumOrdering::Default, false));
        self.init_search();

        self.albums_grid.connect_activate(|grid, index| {
            let album = Arc::clone(
                (grid.model().unwrap().item(index).unwrap())
                    .downcast_ref::<AlbumObject>()
                    .unwrap()
                    .shared_album(),
            );
            ui_tx().send(UpdateUI::AlbumPage(album)).expect(EXP_RX);
        });

        let fallback_image = fallback_album_image();
        let factory = gtk::SignalListItemFactory::new();
        factory.connect_setup(move |_, list_item| {
            list_item
                .downcast_ref::<gtk::ListItem>()
                .expect("Needs to be ListItem")
                .set_child(Some(&ItemTile::default()));
        });
        factory.connect_bind(move |_, list_item| {
            let list_item = list_item
                .downcast_ref::<gtk::ListItem>()
                .expect("Needs to be ListItem");
            let album_object = list_item
                .item()
                .and_downcast::<AlbumObject>()
                .expect("Needs to be AlbumObject");
            let album_tile = list_item
                .downcast_ref::<gtk::ListItem>()
                .expect("Needs to be ListItem")
                .child()
                .and_downcast::<ItemTile>()
                .expect("Needs to be ItemTile");

            album_tile.set_info(&album_object.album(), &album_object.artist());
            if let Some(artwork) = album_object.artwork() {
                album_tile.set_artwork(&artwork);
            } else {
                album_object.load_artwork();
                album_tile.set_artwork(&fallback_image);
            }

            album_tile.add_binding(
                album_object
                    .bind_property("artwork", &album_tile.imp().image.get(), "paintable")
                    .sync_create()
                    .build(),
            );
        });
        factory.connect_unbind(|_, list_item| {
            let list_item = list_item
                .downcast_ref::<gtk::ListItem>()
                .expect("Needs to be ListItem");
            let object = list_item
                .item()
                .and_downcast::<AlbumObject>()
                .expect("Needs to be AlbumObject");
            let album_tile = list_item
                .downcast_ref::<gtk::ListItem>()
                .expect("Needs to be ListItem")
                .child()
                .and_downcast::<ItemTile>()
                .expect("Needs to be ItemTile");

            album_tile.reset_bindings();
            object.unload_artwork();
        });

        self.albums_grid.set_factory(Some(&factory));
    }
}
impl WidgetImpl for AlbumsPage {}
impl NavigationPageImpl for AlbumsPage {}
