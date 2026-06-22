use adw::{prelude::*, subclass::prelude::*};
use core::cell::{Cell, OnceCell, RefCell};
use core::sync::atomic::Ordering;
use gtk::CompositeTemplate;
use gtk::{gdk, gio, glib};
use rand::random_range;
use std::rc::Rc;
use std::sync::Arc;
use std::time::{Duration, Instant};

use crate::UI_TIMEOUT;
use crate::excuses::{EXP_INIT, EXP_RX};
use crate::library::{Songs, ToQueue};
use crate::player::{PlayerRequest, player_tx};
use crate::ui::{ItemRow, SongObject, SongOrdering, SortConfig};
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
    pub search_entry: TemplateChild<gtk::SearchEntry>,
    search_query: Rc<RefCell<String>>,

    songs: RefCell<Vec<SongObject>>,
    filter: Rc<RefCell<gtk::CustomFilter>>,
    sorter: Rc<RefCell<gtk::CustomSorter>>,

    sort_mode: OnceCell<SortConfig<SongOrdering>>,

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
        let async_timeout = Duration::from_millis(1000 / 60);
        let mut async_timer = Instant::now();

        let mut song_objects = Vec::with_capacity(songs.len());
        for (index, song) in songs.iter().enumerate() {
            song_objects.push(SongObject::new(index as u32, Arc::clone(song)));
            if async_timer.elapsed() > async_timeout {
                glib::timeout_future(wait).await;
                async_timer = Instant::now();
            }
        }
        let model = gio::ListStore::new::<SongObject>();
        model.extend_from_slice(&song_objects);
        self.update_sort_fields(&model).await;
        self.songs.replace(song_objects);
        glib::timeout_future(wait).await;

        let query = Rc::clone(&self.search_query);
        let filter = gtk::CustomFilter::new(move |object| {
            let song_object = object.downcast_ref::<SongObject>().unwrap();
            let query = &query.borrow().to_lowercase();
            let score = search::query_score(query, &song_object.song().to_lowercase())
                .max(search::query_score(query, &song_object.artist().to_lowercase()) / 4.0);
            song_object.set_rank(score);
            score > 0.01
        });
        let filter_model = gtk::FilterListModel::new(Some(model), Some(filter.clone()));
        self.filter.replace(filter);
        glib::timeout_future(wait).await;

        let sort_mode = *self.sort_mode.get().unwrap();
        let sorter = gtk::CustomSorter::new(move |object_a, object_b| {
            let song_a = object_a.downcast_ref::<SongObject>().unwrap();
            let song_b = object_b.downcast_ref::<SongObject>().unwrap();
            song_a.order_cmp(song_b, sort_mode)
        });
        let sort_model = gtk::SortListModel::new(Some(filter_model), Some(sorter.clone()));
        self.sorter.replace(sorter);
        glib::timeout_future(wait).await;

        self.songs_grid
            .set_model(Some(&gtk::NoSelection::new(Some(sort_model))));

        // Restore the previous scroll position if already mapped, otherwise it
        // will be restored when mapped (see `connect_map` in `constructed`)
        if self.songs_grid.is_mapped() {
            self.restore_scroll_pos();
        }
    }

    #[inline]
    pub fn assign_artwork(&self, index: usize, artwork: Option<&gdk::Texture>) {
        let songs = self.songs.borrow();
        if index < songs.len() {
            songs[index].set_property("artwork", artwork);
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
                let song = item.downcast_ref::<SongObject>().unwrap();
                let shared_song = song.shared_song();
                let info = shared_song.info();
                let info = info.user();

                song.set_random(random_range(0..u64::MAX));
                song.set_played(info.play_count as u64);
                song.set_rating(match info.rating {
                    0 => 3,
                    n => n,
                });
                song.set_modified(info.modified);
                song.set_added(info.added);
            }
            drop(item);

            if async_timer.elapsed() > UI_TIMEOUT {
                glib::timeout_future(wait).await;
                async_timer = Instant::now();
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
            self.update_sort_fields(model).await;
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

        // Restore the previous scroll position after reload
        // Setting the scroll position must be done when mapped; if it wasn't
        // set in `load_songs`, it is restored in `connect_map` instead.
        self.songs_grid.connect_map(glib::clone!(
            #[weak(rename_to=songs_page)]
            self,
            move |_| songs_page.restore_scroll_pos()
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
