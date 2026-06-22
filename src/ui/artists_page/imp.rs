use adw::{prelude::*, subclass::prelude::*};
use core::cell::{Cell, OnceCell, RefCell};
use gtk::CompositeTemplate;
use gtk::{gdk, gio, glib};
use rand::random_range;
use std::rc::Rc;
use std::sync::Arc;
use std::time::{Duration, Instant};

use crate::UI_TIMEOUT;
use crate::excuses::{EXP_INIT, EXP_RX};
use crate::library::{Artists, ToQueue, ToShuffledQueue};
use crate::player::{PlayerRequest, player_tx};
use crate::ui::{ArtistObject, ArtistOrdering, ItemTile, SortConfig};
use crate::ui::{UpdateUI, ui_tx};
use crate::util::search;

#[derive(Default, CompositeTemplate)]
#[template(file = "artists_page.ui")]
pub struct ArtistsPage {
    #[template_child]
    play_button: TemplateChild<adw::SplitButton>,
    #[template_child]
    sort_button: TemplateChild<adw::SplitButton>,

    #[template_child]
    view_stack: TemplateChild<adw::ViewStack>,
    #[template_child]
    artists_grid: TemplateChild<gtk::GridView>,

    #[template_child]
    pub search_entry: TemplateChild<gtk::SearchEntry>,
    search_query: Rc<RefCell<String>>,

    artists: RefCell<Vec<ArtistObject>>,
    filter: Rc<RefCell<gtk::CustomFilter>>,
    sorter: Rc<RefCell<gtk::CustomSorter>>,

    sort_mode: OnceCell<SortConfig<ArtistOrdering>>,

    shuffle: Cell<bool>,
    pending_scroll_pos: Cell<Option<f64>>,
}

#[gtk::template_callbacks]
impl ArtistsPage {
    #[template_callback]
    pub fn handle_search_changed(&self) {
        self.search_query
            .replace(self.search_entry.text().to_string());
        self.filter.borrow().changed(gtk::FilterChange::Different);
        self.sorter.borrow().changed(gtk::SorterChange::Different);
    }
    #[template_callback]
    pub fn handle_activate(&self) {
        self.artists_grid.grab_focus();
    }
    #[template_callback]
    pub fn handle_stop_search(&self) {
        self.search_entry.set_text("");
        self.search_query.take();
        self.artists_grid.grab_focus();
    }

    #[template_callback]
    pub fn handle_play_now(&self) {
        let model = self.artists_grid.model().expect(EXP_INIT);
        let n_items = model.n_items();
        let mut artists = Vec::with_capacity(n_items as usize);

        for i in 0..n_items {
            artists.push(Arc::clone(
                (model.item(i).unwrap().downcast_ref::<ArtistObject>())
                    .unwrap()
                    .shared_artist(),
            ));
        }

        let player_tx = player_tx();
        player_tx
            .send(PlayerRequest::LoadQueue {
                queue: match self.shuffle.get() {
                    true => artists.to_shuffled_queue(),
                    false => artists.to_queue(),
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
    pub async fn load_artists(&self, artists: &Artists) {
        if artists.is_empty() {
            self.artists_grid.set_model(None::<&gtk::NoSelection>);
            self.view_stack.set_visible_child_name("empty");
            return;
        }
        self.view_stack.set_visible_child_name("artists");
        self.remember_scroll_pos();

        // The timers are used to reduce major UI stutters
        // by turning them into multiple smaller ones
        let wait = Duration::from_millis(10);
        let mut async_timer = Instant::now();

        let mut artist_objects = Vec::with_capacity(artists.len());
        for (index, artist) in artists.iter().enumerate() {
            // NOTE: Scope is required due to a Clippy warning false-positive
            // when `MutexGuard`s are explicitly dropped before the `await` point
            // Issue link: <https://github.com/rust-lang/rust-clippy/issues/6446>
            {
                let artist_locked = artist.lock().unwrap();
                artist_objects.push(ArtistObject::new(
                    index as u32,
                    artist_locked.name(),
                    artist_locked.albums().len() as u64,
                    Arc::clone(artist),
                ));
            }

            if async_timer.elapsed() > UI_TIMEOUT {
                glib::timeout_future(wait).await;
                async_timer = Instant::now();
            }
        }
        let model = gio::ListStore::new::<ArtistObject>();
        model.extend_from_slice(&artist_objects);
        self.update_sort_fields(&model).await;
        self.artists.replace(artist_objects);
        glib::timeout_future(wait).await;

        let query = Rc::clone(&self.search_query);
        let filter = gtk::CustomFilter::new(move |object| {
            let artist_object = object.downcast_ref::<ArtistObject>().unwrap();
            let score = search::query_score(
                &query.borrow().to_lowercase(),
                &artist_object.artist().to_lowercase(),
            );
            artist_object.set_rank(score);
            score > 0.01
        });
        let filter_model = gtk::FilterListModel::new(Some(model), Some(filter.clone()));
        self.filter.replace(filter);
        glib::timeout_future(wait).await;

        let sort_mode = *self.sort_mode.get().unwrap();
        let sorter = gtk::CustomSorter::new(move |object_a, object_b| {
            let artist_a = object_a.downcast_ref::<ArtistObject>().unwrap();
            let artist_b = object_b.downcast_ref::<ArtistObject>().unwrap();
            artist_a.order_cmp(artist_b, sort_mode)
        });
        let sort_model = gtk::SortListModel::new(Some(filter_model), Some(sorter.clone()));
        self.sorter.replace(sorter);
        glib::timeout_future(wait).await;

        self.artists_grid
            .set_model(Some(&gtk::NoSelection::new(Some(sort_model))));

        // Restore the previous scroll position if already mapped, otherwise it
        // will be restored when mapped (see `connect_map` in `constructed`)
        if self.artists_grid.is_mapped() {
            self.restore_scroll_pos();
        }
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
                let artist = item.downcast_ref::<ArtistObject>().unwrap();
                let shared_artist = artist.shared_artist();
                let artist_locked = shared_artist.lock().unwrap();
                let album_locked = artist_locked.newest_album().lock().unwrap();
                let song = album_locked.first_song();
                let info = song.info();
                let info = info.user();

                artist.set_random(random_range(0..u64::MAX));
                artist.set_modified(info.modified);
                artist.set_added(info.added);
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
    pub fn assign_artwork(&self, index: usize, artwork: Option<gdk::Texture>) {
        let artists = self.artists.borrow();
        if index < artists.len() {
            artists[index].set_property("artwork", artwork);
        }
    }

    #[template_callback]
    pub fn handle_reverse_sort(&self) {
        self.remember_scroll_pos();
        let reversed = self.sort_mode.get().expect(EXP_INIT).reversed;
        let reverse = !reversed.get();
        reversed.set(reverse);
        self.sorter.borrow().changed(gtk::SorterChange::Inverted);
        self.sort_button.set_icon_name(match reverse {
            true => "view-sort-ascending-symbolic",
            false => "view-sort-descending-symbolic",
        });
        self.restore_scroll_pos();
    }
    #[inline]
    pub async fn set_sort_mode(&self, sort_mode: ArtistOrdering) {
        self.remember_scroll_pos();
        let ordering = self.sort_mode.get().expect(EXP_INIT).ordering;
        ordering.replace(sort_mode);
        self.sorter.borrow().changed(gtk::SorterChange::Different);
        if let Some(model) = &self.artists_grid.model() {
            self.update_sort_fields(model).await;
        }
        self.restore_scroll_pos();
    }
    #[inline]
    #[must_use]
    pub fn get_sort_mode(&self) -> &SortConfig<ArtistOrdering> {
        self.sort_mode.get().expect(EXP_INIT)
    }

    #[inline]
    fn remember_scroll_pos(&self) {
        self.pending_scroll_pos.set(Some(
            self.artists_grid.vadjustment().map_or(0.0, |v| v.value()),
        ));
    }
    #[inline]
    fn restore_scroll_pos(&self) {
        if let Some(scroll_pos) = self.pending_scroll_pos.take()
            && let Some(vadjustment) = self.artists_grid.vadjustment()
        {
            glib::idle_add_local_once(move || vadjustment.set_value(scroll_pos));
        }
    }

    #[inline]
    pub const fn uninit(&self) {
        // for artist in self.artists.take() {
        //     artist.imp().is_visible.store(false, Ordering::Relaxed);
        // }
    }
}

#[glib::object_subclass]
impl ObjectSubclass for ArtistsPage {
    const NAME: &str = "MellowArtistsPage";
    type Type = super::ArtistsPage;
    type ParentType = adw::NavigationPage;

    fn class_init(class: &mut Self::Class) {
        class.bind_template();
        class.bind_template_callbacks();
    }

    fn instance_init(obj: &glib::subclass::InitializingObject<Self>) {
        obj.init_template();
    }
}
impl ObjectImpl for ArtistsPage {
    fn constructed(&self) {
        let _ = self
            .sort_mode
            .set(SortConfig::new(ArtistOrdering::Default, false));

        self.artists_grid.connect_activate(|grid, index| {
            let artist = Arc::clone(
                (grid.model().unwrap().item(index).unwrap())
                    .downcast_ref::<ArtistObject>()
                    .unwrap()
                    .shared_artist(),
            );
            ui_tx().send(UpdateUI::ArtistPage(artist)).expect(EXP_RX);
        });

        // Restore the previous scroll position after reload
        // Setting the scroll position must be done when mapped; if it wasn't
        // set in `load_artists`, it is restored in `connect_map` instead.
        self.artists_grid.connect_map(glib::clone!(
            #[weak(rename_to=artists_page)]
            self,
            move |_| artists_page.restore_scroll_pos()
        ));

        // let fallback_image = fallback_artist_image();
        let factory = gtk::SignalListItemFactory::new();
        factory.connect_setup(move |_, list_item| {
            let artist_tile = ItemTile::builder()
                .show_artwork(false)
                .width_request(180)
                .height_request(-1)
                .margin_bottom(8)
                .margin_top(8)
                .build();
            list_item
                .downcast_ref::<gtk::ListItem>()
                .expect("Needs to be ListItem")
                .set_child(Some(&artist_tile));
        });
        factory.connect_bind(move |_, list_item| {
            let list_item = list_item
                .downcast_ref::<gtk::ListItem>()
                .expect("Needs to be ListItem");
            let artist_object = list_item
                .item()
                .and_downcast::<ArtistObject>()
                .expect("Needs to be ArtistObject");
            let artist_tile = list_item
                .downcast_ref::<gtk::ListItem>()
                .expect("Needs to be ListItem")
                .child()
                .and_downcast::<ItemTile>()
                .expect("Needs to be ItemTile");

            artist_tile.set_info(
                &artist_object.artist(),
                &format!("Albums: {}", artist_object.albums()),
            );
            // if let Some(artwork) = artist_object.artwork() {
            //     artist_tile.set_artwork(&artwork);
            // } else {
            //     artist_object.load_artwork();
            //     artist_tile.set_artwork(&fallback_image);
            // }

            // artist_tile.add_binding(
            //     artist_object
            //         .bind_property("artwork", &artist_tile.imp().image.get(), "paintable")
            //         .sync_create()
            //         .build(),
            // );
        });
        factory.connect_unbind(|_, list_item| {
            let list_item = list_item
                .downcast_ref::<gtk::ListItem>()
                .expect("Needs to be ListItem");
            let artist_object = list_item
                .item()
                .and_downcast::<ArtistObject>()
                .expect("Needs to be AlbumObject");
            let artist_tile = list_item
                .downcast_ref::<gtk::ListItem>()
                .expect("Needs to be ListItem")
                .child()
                .and_downcast::<ItemTile>()
                .expect("Needs to be ItemTile");

            artist_tile.reset_bindings();
            artist_object.unload_artwork();
        });

        self.artists_grid.set_factory(Some(&factory));
    }
}
impl WidgetImpl for ArtistsPage {}
impl NavigationPageImpl for ArtistsPage {}
