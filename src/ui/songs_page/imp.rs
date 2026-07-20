use adw::{prelude::*, subclass::prelude::*};
use core::cell::{Cell, OnceCell, RefCell};
use core::cmp;
use core::sync::atomic::Ordering;
use fastrand;
use gtk::CompositeTemplate;
use gtk::{gdk, gio, glib};
use std::rc::Rc;
use std::sync::Arc;
use std::time::{Duration, Instant};

use crate::UI_TIMEOUT;
use crate::excuses::{EXP_INIT, EXP_RX};
use crate::library::tag_list::{self, Tags};
use crate::library::{Songs, ToQueue};
use crate::player::{PlayerRequest, player_tx};
use crate::ui::song_object::SongFilters;
use crate::ui::{FilterMode, ItemRow, SongObject, SongOrdering, SortConfig};
use crate::ui::{UpdateUI, fallback_song_image, ui_tx};
use crate::util::search;

#[derive(Default, CompositeTemplate)]
#[template(file = "songs_page.ui")]
pub struct SongsPage {
    #[template_child]
    play_button: TemplateChild<adw::SplitButton>,
    #[template_child]
    sort_button: TemplateChild<adw::SplitButton>,

    #[template_child]
    view_stack: TemplateChild<adw::ViewStack>,
    #[template_child]
    songs_grid: TemplateChild<gtk::GridView>,

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
    songs: RefCell<Vec<SongObject>>,
    filter: RefCell<gtk::CustomFilter>,
    sorter: RefCell<gtk::CustomSorter>,

    sort_mode: OnceCell<SortConfig<SongOrdering>>,
    song_filters: Rc<RefCell<SongFilters>>,

    shuffle: Cell<bool>,
    pending_scroll_pos: Cell<Option<f64>>,
}

#[gtk::template_callbacks]
impl SongsPage {
    #[template_callback]
    pub fn handle_search_changed(&self) {
        self.search_query
            .replace(self.search_entry.text().to_string());
        self.filter.borrow().changed(gtk::FilterChange::Different);
        self.sorter.borrow().changed(gtk::SorterChange::Different);
    }
    #[template_callback]
    pub fn handle_activate(&self) {
        self.songs_grid.grab_focus();
    }
    #[template_callback]
    pub fn handle_stop_search(&self) {
        self.search_entry.set_text("");
        self.search_query.take();
        self.songs_grid.grab_focus();
    }
    #[template_callback]
    pub fn handle_filters_changed(&self) {
        let mut filters = self.song_filters.borrow_mut();

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
        let model = self.songs_grid.model().expect(EXP_INIT);
        let n_items = model.n_items();
        let mut songs = Vec::with_capacity(n_items as usize);

        for i in 0..n_items {
            songs.push(
                (model.item(i).unwrap().downcast_ref::<SongObject>())
                    .unwrap()
                    .shared_song(),
            );
        }

        let player_tx = player_tx();
        (player_tx.send(PlayerRequest::LoadQueue {
            queue: songs.to_queue(),
            shuffled: match self.shuffle.get() {
                true => Some(vec![]),
                false => None,
            },
            track: 0,
        }))
        .expect(EXP_RX);
        let _ = player_tx.send(PlayerRequest::TogglePlay(Some(true)));
        let ui_tx = ui_tx();
        ui_tx.send(UpdateUI::OpenSheet(false)).expect(EXP_RX);
        ui_tx.send(UpdateUI::FocusPlaying).expect(EXP_RX);
    }

    pub fn update_tag_filter_list(&self) {
        self.filtered_tags.remove_all();

        let mut song_filters = self.song_filters.borrow_mut();
        let mut new_tags = Vec::with_capacity(song_filters.tags.len());

        // TODO: When there are no tags available in the library, either show a message
        // or hide the tag filters section in the interface entirely
        for tag in tag_list::read_global_tags().tag_names() {
            let toggle_button = gtk::ToggleButton::builder().label(tag).build();

            // Re-select items which were previously selected
            for (i, selected_tag) in song_filters.tags.iter().enumerate() {
                if selected_tag == tag {
                    toggle_button.set_active(true);
                    new_tags.push(song_filters.tags.get_mut().remove(i));
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
                        true => page.song_filters.borrow_mut().tags.add(tag.clone()),
                        false => page.song_filters.borrow_mut().tags.remove(&tag),
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

        song_filters.tags = Tags::from(new_tags);
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
    pub async fn load_songs(&self, songs: &Songs) {
        let id = self.contents_id.get().wrapping_add(1);
        self.contents_id.set(id);
        if songs.is_empty() {
            self.songs_grid.set_model(None::<&gtk::NoSelection>);
            self.view_stack.set_visible_child_name("empty");
            return;
        }
        self.view_stack.set_visible_child_name("songs");
        self.remember_scroll_pos();

        // The timers are used to reduce major UI stutters
        // by turning them into multiple smaller ones
        let wait = Duration::from_millis(10);
        let mut async_timer = Instant::now();

        let mut song_objects = Vec::with_capacity(songs.len());
        for (index, song) in songs.iter().enumerate() {
            match SongObject::new(index as u32, Arc::clone(song)) {
                Ok(song_object) => song_objects.push(song_object),
                Err(_) => {
                    #[cfg(feature = "startup-logs")]
                    eprintln!(
                        "WARNING: Song info was not loaded; refusing to load on the main thread\n{}",
                        "If another function call has succeeded afterwards, this warning can be ignored"
                    );
                    return;
                }
            }
            if async_timer.elapsed() > UI_TIMEOUT {
                glib::timeout_future(wait).await;
                async_timer = Instant::now();
                if self.contents_id.get() != id {
                    #[cfg(feature = "startup-logs")]
                    println!(
                        "Songs page contents ID changed during objects construction - stopping"
                    );
                    return;
                }
            }
        }
        let model = gio::ListStore::new::<SongObject>();
        model.extend_from_slice(&song_objects);

        // Restore the previous scroll position and update sort fields if already mapped,
        // otherwise it will happen when mapped (see `connect_map` in `constructed`)
        if self.songs_grid.is_mapped() {
            self.update_sort_fields(&model, id).await;
            if self.contents_id.get() != id {
                #[cfg(feature = "startup-logs")]
                println!("Songs page contents ID changed - stopping");
                return;
            }
            self.restore_scroll_pos();
        }

        self.songs.replace(song_objects);

        let query = Rc::clone(&self.search_query);
        let song_filters = Rc::clone(&self.song_filters);
        let filter = gtk::CustomFilter::new(move |object| {
            let song_object = object.downcast_ref::<SongObject>().unwrap();
            let query = &query.borrow().to_lowercase();
            let score = search::query_score(query, &song_object.song().to_lowercase())
                .max(search::query_score(query, &song_object.artist().to_lowercase()) / 4.0);
            song_object.set_rank(score);
            score > 0.01 && song_filters.borrow_mut().filter(song_object)
        });
        let filter_model = gtk::FilterListModel::new(Some(model), Some(filter.clone()));
        self.filter.replace(filter);

        let sort_mode = *self.sort_mode.get().unwrap();
        let sorter = gtk::CustomSorter::new(move |object_a, object_b| {
            let song_a = object_a.downcast_ref::<SongObject>().unwrap();
            let song_b = object_b.downcast_ref::<SongObject>().unwrap();
            song_a.order_cmp(song_b, sort_mode)
        });
        let sort_model = gtk::SortListModel::new(Some(filter_model), Some(sorter.clone()));
        self.sorter.replace(sorter);

        self.songs_grid
            .set_model(Some(&gtk::NoSelection::new(Some(sort_model))));

        #[cfg(feature = "startup-logs")]
        println!("Songs page loaded");
    }

    #[inline]
    pub fn assign_artwork(&self, index: usize, artwork: Option<&gdk::Texture>) {
        let songs = self.songs.borrow();
        if index < songs.len() {
            songs[index].set_property("artwork", artwork);
        }
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
                let song = item.downcast_ref::<SongObject>().unwrap();
                let shared_song = song.shared_song();
                let info = shared_song.info();
                let info = info.user();

                song.set_random(fastrand::u64(0..u64::MAX));
                song.set_played(info.play_count as u64);
                song.set_stars(info.rating.stars());
                song.set_rating(match info.rating.as_raw() {
                    0 => 3,
                    n => n,
                });
                song.set_modified(info.modified);
                song.set_added(info.added);
                song.set_tags(info.tags().to_vec());
            }
            drop(item);

            if async_timer.elapsed() > UI_TIMEOUT {
                glib::timeout_future(wait).await;
                async_timer = Instant::now();
                if self.contents_id.get() != id {
                    #[cfg(feature = "startup-logs")]
                    println!(
                        "Songs page contents ID changed while updating sort fields - stopping"
                    );
                    return;
                }
            }

            i += 1;
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
    pub async fn set_sort_mode(&self, sort_mode: SongOrdering) {
        self.remember_scroll_pos();
        let ordering = self.sort_mode.get().expect(EXP_INIT).ordering;
        ordering.replace(sort_mode);
        self.sorter.borrow().changed(gtk::SorterChange::Different);
        if let Some(model) = &self.songs_grid.model() {
            self.update_sort_fields(model, self.contents_id.get()).await;
        }
        self.restore_scroll_pos();
    }
    #[inline]
    #[must_use]
    pub fn get_sort_mode(&self) -> &SortConfig<SongOrdering> {
        self.sort_mode.get().expect(EXP_INIT)
    }

    #[inline]
    fn remember_scroll_pos(&self) {
        self.pending_scroll_pos.set(Some(
            self.songs_grid.vadjustment().map_or(0.0, |v| v.value()),
        ));
    }
    #[inline]
    fn restore_scroll_pos(&self) {
        if let Some(scroll_pos) = self.pending_scroll_pos.take()
            && let Some(vadjustment) = self.songs_grid.vadjustment()
        {
            glib::idle_add_local_once(move || vadjustment.set_value(scroll_pos));
        }
    }

    pub fn uninit(&self) {
        for song in self.songs.take() {
            song.imp().is_visible.store(false, Ordering::Release);
        }
    }
}

#[glib::object_subclass]
impl ObjectSubclass for SongsPage {
    const NAME: &str = "MellowSongsPage";
    type Type = super::SongsPage;
    type ParentType = adw::NavigationPage;

    fn class_init(class: &mut Self::Class) {
        class.bind_template();
        class.bind_template_callbacks();
    }

    fn instance_init(obj: &glib::subclass::InitializingObject<Self>) {
        obj.init_template();
    }
}
impl ObjectImpl for SongsPage {
    fn constructed(&self) {
        let _ = self
            .sort_mode
            .set(SortConfig::new(SongOrdering::Default, false));

        self.songs_grid.connect_activate(|grid, index| {
            let index = (grid.model().unwrap().item(index).unwrap())
                .downcast_ref::<SongObject>()
                .unwrap()
                .index();
            (ui_tx().send(UpdateUI::SongPageByIndex(index as usize))).expect(EXP_RX);
        });

        // Restore the previous scroll position if pending, and update sort fields
        // Setting the scroll position must be done when mapped; if it wasn't
        // set in `load_songs`, it is restored in `connect_map` instead.
        self.songs_grid.connect_map(glib::clone!(
            #[weak(rename_to=songs_page)]
            self,
            move |_| {
                songs_page.restore_scroll_pos();
                glib::spawn_future_local(async move {
                    songs_page
                        .update_sort_fields(
                            &songs_page.songs_grid.model().expect(EXP_INIT),
                            songs_page.contents_id.get(),
                        )
                        .await;
                    songs_page.update_tag_filter_list();
                    songs_page.handle_filters_changed();
                });
            }
        ));

        self.filter_mode.connect_active_notify(glib::clone!(
            #[weak(rename_to=songs_page)]
            self,
            move |_| songs_page.handle_filters_changed()
        ));

        let fallback_image = fallback_song_image();
        let factory = gtk::SignalListItemFactory::new();
        factory.connect_setup(move |_, list_item| {
            list_item
                .downcast_ref::<gtk::ListItem>()
                .expect("Needs to be ListItem")
                .set_child(Some(&ItemRow::default()));
        });
        factory.connect_bind(move |_, list_item| {
            let list_item = list_item
                .downcast_ref::<gtk::ListItem>()
                .expect("Needs to be ListItem");
            let song_object = list_item
                .item()
                .and_downcast::<SongObject>()
                .expect("Needs to be SongObject");
            let song_row = list_item
                .downcast_ref::<gtk::ListItem>()
                .expect("Needs to be ListItem")
                .child()
                .and_downcast::<ItemRow>()
                .expect("Needs to be ItemRow");

            song_row.set_info(&song_object.song(), &song_object.artist());
            if let Some(artwork) = song_object.artwork() {
                song_row.set_artwork(&artwork);
            } else {
                song_object.load_artwork();
                song_row.set_artwork(&fallback_image);
            }

            song_row.add_binding(
                song_object
                    .bind_property("artwork", &song_row.imp().image.get(), "paintable")
                    .sync_create()
                    .build(),
            );
        });
        factory.connect_unbind(|_, list_item| {
            let list_item = list_item
                .downcast_ref::<gtk::ListItem>()
                .expect("Needs to be ListItem");
            let song_object = list_item
                .item()
                .and_downcast::<SongObject>()
                .expect("Needs to be AlbumObject");
            let song_row = list_item
                .downcast_ref::<gtk::ListItem>()
                .expect("Needs to be ListItem")
                .child()
                .and_downcast::<ItemRow>()
                .expect("Needs to be ItemTile");

            song_row.reset_bindings();
            song_object.unload_artwork();
        });

        self.songs_grid.set_factory(Some(&factory));
    }
}
impl WidgetImpl for SongsPage {}
impl NavigationPageImpl for SongsPage {}
