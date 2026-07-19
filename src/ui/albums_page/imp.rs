use adw::{prelude::*, subclass::prelude::*};
use core::cell::{Cell, OnceCell, RefCell};
use core::cmp;
use core::sync::atomic::Ordering;
use gtk::CompositeTemplate;
use gtk::{gdk, gio, glib};
use rand::random_range;
use std::rc::Rc;
use std::sync::Arc;
use std::time::{Duration, Instant};

use crate::UI_TIMEOUT;
use crate::excuses::{EXP_INIT, EXP_RX};
use crate::library::tag_list::{self, Tags};
use crate::library::{Albums, ToQueue, ToShuffledQueue};
use crate::player::{PlayerRequest, player_tx};
use crate::ui::album_object::AlbumFilters;
use crate::ui::{AlbumObject, AlbumOrdering, FilterMode, ItemTile, SortConfig};
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
    filter_mode: TemplateChild<adw::ToggleGroup>,
    #[template_child]
    filtered_tags: TemplateChild<adw::WrapBox>,

    #[template_child]
    rating_checkbox: TemplateChild<gtk::CheckButton>,
    #[template_child]
    rating_spin_row: TemplateChild<adw::SpinRow>,
    #[template_child]
    rating_condition: TemplateChild<gtk::DropDown>,

    #[template_child]
    play_count_checkbox: TemplateChild<gtk::CheckButton>,
    #[template_child]
    play_count_spin_row: TemplateChild<adw::SpinRow>,
    #[template_child]
    play_count_condition: TemplateChild<gtk::DropDown>,

    #[template_child]
    year_checkbox: TemplateChild<gtk::CheckButton>,
    #[template_child]
    year_spin_row: TemplateChild<adw::SpinRow>,
    #[template_child]
    year_condition: TemplateChild<gtk::DropDown>,

    #[template_child]
    pub search_entry: TemplateChild<gtk::SearchEntry>,
    search_query: Rc<RefCell<String>>,

    contents_id: Cell<u8>,
    albums: RefCell<Vec<AlbumObject>>,
    filter: RefCell<gtk::CustomFilter>,
    sorter: RefCell<gtk::CustomSorter>,

    sort_mode: OnceCell<SortConfig<AlbumOrdering>>,
    album_filters: Rc<RefCell<AlbumFilters>>,

    shuffle: Cell<bool>,
    pending_scroll_pos: Cell<Option<f64>>,
}

#[gtk::template_callbacks]
impl AlbumsPage {
    #[template_callback]
    pub fn handle_search_changed(&self) {
        self.search_query
            .replace(self.search_entry.text().to_string());
        self.filter.borrow().changed(gtk::FilterChange::Different);
        self.sorter.borrow().changed(gtk::SorterChange::Different);
    }
    #[template_callback]
    pub fn handle_activate(&self) {
        self.albums_grid.grab_focus();
    }
    #[template_callback]
    pub fn handle_stop_search(&self) {
        self.search_entry.set_text("");
        self.search_query.take();
        self.albums_grid.grab_focus();
    }
    #[template_callback]
    pub fn handle_filters_changed(&self) {
        let mut filters = self.album_filters.borrow_mut();

        filters.filter_mode = match self.filter_mode.active() {
            0 => FilterMode::Inclusive,
            1 => FilterMode::Exclusive,
            _ => unimplemented!(),
        };
        filters.rating = match self.rating_checkbox.is_active() {
            true => Some((
                match self.rating_condition.selected() {
                    0 => cmp::Ordering::Greater,
                    1 => cmp::Ordering::Less,
                    _ => unimplemented!(),
                },
                self.rating_spin_row.value() as u8,
            )),
            false => None,
        };
        filters.play_count = match self.play_count_checkbox.is_active() {
            true => Some((
                match self.play_count_condition.selected() {
                    0 => cmp::Ordering::Greater,
                    1 => cmp::Ordering::Less,
                    _ => unimplemented!(),
                },
                self.play_count_spin_row.value() as u64,
            )),
            false => None,
        };
        filters.year = match self.year_checkbox.is_active() {
            true => Some((
                match self.year_condition.selected() {
                    0 => cmp::Ordering::Greater,
                    1 => cmp::Ordering::Less,
                    _ => unimplemented!(),
                },
                self.year_spin_row.value() as u32,
            )),
            false => None,
        };

        drop(filters);
        self.remember_scroll_pos();
        self.filter.borrow().changed(gtk::FilterChange::Different);
        self.restore_scroll_pos();
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

    pub fn update_tag_filter_list(&self) {
        self.filtered_tags.remove_all();

        let mut album_filters = self.album_filters.borrow_mut();
        let mut new_tags = Vec::with_capacity(album_filters.tags.len());

        // TODO: When there are no tags available in the library, either show a message
        // or hide the tag filters section in the interface entirely
        for tag in tag_list::read_global_tags().tag_names() {
            let toggle_button = gtk::ToggleButton::builder().label(tag).build();

            // Re-select items which were previously selected
            for (i, selected_tag) in album_filters.tags.iter().enumerate() {
                if selected_tag == tag {
                    toggle_button.set_active(true);
                    new_tags.push(album_filters.tags.get_mut().remove(i));
                    break;
                }
            }

            // Update filters when toggling them in the UI
            let tag = tag.to_owned();
            toggle_button.connect_active_notify(glib::clone!(
                #[weak(rename_to = page)]
                self,
                move |toggle| {
                    match toggle.is_active() {
                        true => page.album_filters.borrow_mut().tags.add(tag.clone()),
                        false => page.album_filters.borrow_mut().tags.remove(&tag),
                    }

                    glib::idle_add_local_once(move || {
                        page.remember_scroll_pos();
                        page.filter.borrow().changed(gtk::FilterChange::Different);
                        page.restore_scroll_pos();
                    });
                }
            ));

            self.filtered_tags.append(&toggle_button);
        }

        album_filters.tags = Tags::from(new_tags);
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
        let id = self.contents_id.get().wrapping_add(1);
        self.contents_id.set(id);
        if albums.is_empty() {
            self.albums_grid.set_model(None::<&gtk::NoSelection>);
            self.view_stack.set_visible_child_name("empty");
            return;
        }
        self.view_stack.set_visible_child_name("albums");
        self.remember_scroll_pos();

        // TODO: Add a proper callback for when library tags are updated instead of calling this here
        self.update_tag_filter_list();

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
                if self.contents_id.get() != id {
                    println!("Library page contents ID changed - stopping");
                    return;
                }
            }
        }
        let model = gio::ListStore::new::<AlbumObject>();
        model.extend_from_slice(&album_objects);
        self.update_sort_fields(&model, id).await;
        if self.contents_id.get() != id {
            println!("Library page contents ID changed - stopping");
            return;
        }
        self.albums.replace(album_objects);
        glib::timeout_future(wait).await;
        if self.contents_id.get() != id {
            println!("Library page contents ID changed - stopping");
            return;
        }

        let query = Rc::clone(&self.search_query);
        let album_filters = Rc::clone(&self.album_filters);
        let filter = gtk::CustomFilter::new(move |object| {
            let album_object = object.downcast_ref::<AlbumObject>().unwrap();
            let query = &query.borrow().to_lowercase();
            let score = search::query_score(query, &album_object.album().to_lowercase())
                .max(search::query_score(query, &album_object.artist().to_lowercase()) / 4.0);
            album_object.set_rank(score);
            score > 0.01 && album_filters.borrow().filter(album_object)
        });
        let filter_model = gtk::FilterListModel::new(Some(model), Some(filter.clone()));
        self.filter.replace(filter);
        glib::timeout_future(wait).await;
        if self.contents_id.get() != id {
            println!("Library page contents ID changed - stopping");
            return;
        }

        let sort_mode = *self.sort_mode.get().unwrap();
        let sorter = gtk::CustomSorter::new(move |object_a, object_b| {
            let album_a = object_a.downcast_ref::<AlbumObject>().unwrap();
            let album_b = object_b.downcast_ref::<AlbumObject>().unwrap();
            album_a.order_cmp(album_b, sort_mode)
        });
        let sort_model = gtk::SortListModel::new(Some(filter_model), Some(sorter.clone()));
        self.sorter.replace(sorter);
        glib::timeout_future(wait).await;
        if self.contents_id.get() != id {
            println!("Library page contents ID changed - stopping");
            return;
        }

        self.albums_grid
            .set_model(Some(&gtk::NoSelection::new(Some(sort_model))));

        // Restore the previous scroll position if already mapped, otherwise it
        // will be restored when mapped (see `connect_map` in `constructed`)
        if self.albums_grid.is_mapped() {
            self.restore_scroll_pos();
        }

        #[cfg(feature = "startup-logs")]
        println!("Albums page loaded");
    }

    #[inline]
    pub async fn update_sort_fields<M>(&self, model: &M, id: u8)
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

                album.set_random(random_range(0..=u64::MAX));
                album.set_played(album_locked.average_play_count());
                album.set_stars(album_locked.average_rating(0.0));
                album.set_rating(album_locked.sort_rating(3.0));
                album.set_tags(
                    (album_locked.user_info().tags.tag_names_owned()).collect::<Vec<String>>(),
                );

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
                if self.contents_id.get() != id {
                    println!("Library page contents ID changed - stopping");
                    return;
                }
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
    pub async fn set_sort_mode(&self, sort_mode: AlbumOrdering) {
        self.remember_scroll_pos();
        let ordering = self.sort_mode.get().expect(EXP_INIT).ordering;
        ordering.replace(sort_mode);
        self.sorter.borrow().changed(gtk::SorterChange::Different);
        if let Some(model) = &self.albums_grid.model() {
            self.update_sort_fields(model, self.contents_id.get()).await;
        }
        self.restore_scroll_pos();
    }
    #[inline]
    #[must_use]
    pub fn get_sort_mode(&self) -> &SortConfig<AlbumOrdering> {
        self.sort_mode.get().expect(EXP_INIT)
    }

    #[inline]
    fn remember_scroll_pos(&self) {
        self.pending_scroll_pos.set(Some(
            self.albums_grid.vadjustment().map_or(0.0, |v| v.value()),
        ));
    }
    #[inline]
    fn restore_scroll_pos(&self) {
        if let Some(scroll_pos) = self.pending_scroll_pos.take()
            && let Some(vadjustment) = self.albums_grid.vadjustment()
        {
            glib::idle_add_local_once(move || vadjustment.set_value(scroll_pos));
        }
    }

    pub fn uninit(&self) {
        for album in self.albums.take() {
            album.imp().is_visible.store(false, Ordering::Release);
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

        self.albums_grid.connect_activate(|grid, index| {
            let album = Arc::clone(
                (grid.model().unwrap().item(index).unwrap())
                    .downcast_ref::<AlbumObject>()
                    .unwrap()
                    .shared_album(),
            );
            ui_tx().send(UpdateUI::AlbumPage(album)).expect(EXP_RX);
        });

        // Restore the previous scroll position after reload
        // Setting the scroll position must be done when mapped; if it wasn't
        // set in `load_albums`, it is restored in `connect_map` instead.
        self.albums_grid.connect_map(glib::clone!(
            #[weak(rename_to=albums_page)]
            self,
            move |_| albums_page.restore_scroll_pos()
        ));

        self.filter_mode.connect_active_notify(glib::clone!(
            #[weak(rename_to=songs_page)]
            self,
            move |_| songs_page.handle_filters_changed()
        ));

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
