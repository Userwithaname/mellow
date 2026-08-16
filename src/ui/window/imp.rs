use adw::ApplicationWindow;
use adw::{prelude::*, subclass::prelude::*};
use core::cell::{Cell, OnceCell, RefCell};
use core::hint::cold_path;
use core::time::Duration;
use glib::subclass::InitializingObject;
use gtk::{CompositeTemplate, gio, glib};
use std::sync::Arc;
use std::thread;

use crate::excuses::{ACTION_ERR, EXP_RX};
use crate::library::unload_unused::UsedBy;
use crate::library::{Albums, Artists, SharedAlbum, SharedArtist, SharedSong, Songs, ToQueue};
use crate::library::{Library, LibraryRequest, library_tx};
use crate::mpris::{UpdateMPRIS, mpris_tx};
use crate::music_dir;
use crate::player::QueueItem;
use crate::ui::{
    AlbumPage, AlbumsPage, ArtistPage, ArtistsPage, SongPage, SongsPage, ToastButtonAction,
};
use crate::ui::{LibraryPage, LyricsPage, MainPlayer, SettingsPage, SubpageType};
use crate::ui::{QueuePage, QueueSubpage};
use crate::ui::{UpdateUI, ui_tx};
use crate::util::Forever;

#[derive(Default, CompositeTemplate)]
#[template(file = "window.ui")]
pub struct Window {
    #[template_child]
    pub main_player: TemplateChild<MainPlayer>,

    #[template_child]
    pub drag_overlay: TemplateChild<adw::StatusPage>,
    #[template_child]
    bottom_bar: TemplateChild<gtk::Box>,
    #[template_child]
    sheet: TemplateChild<adw::BottomSheet>,
    #[template_child]
    sheet_content: TemplateChild<adw::ToolbarView>,
    #[template_child]
    view_stack: TemplateChild<adw::ViewStack>,

    // View stack "Library" tab
    #[template_child]
    pub library: TemplateChild<adw::NavigationView>,
    #[template_child]
    pub library_page: TemplateChild<LibraryPage>,
    #[template_child]
    pub songs_page: TemplateChild<SongsPage>,
    #[template_child]
    pub albums_page: TemplateChild<AlbumsPage>,
    #[template_child]
    pub artists_page: TemplateChild<ArtistsPage>,

    pub library_subpages: RefCell<Vec<SubpageType>>,
    pub song_pages: RefCell<Vec<SongPage>>,
    pub album_pages: Forever<RefCell<Vec<AlbumPage>>>,
    pub artist_pages: Forever<RefCell<Vec<ArtistPage>>>,

    // View stack "Playing" tab
    #[template_child]
    pub playing: TemplateChild<adw::NavigationView>,
    #[template_child]
    pub queue_page: TemplateChild<QueuePage>,
    #[template_child]
    queue_subpage: TemplateChild<QueueSubpage>,
    queue_subpage_visible: Cell<bool>,
    #[template_child]
    lyrics_page: TemplateChild<LyricsPage>,

    // View stack "Settings" tab
    #[template_child]
    pub settings_page: TemplateChild<SettingsPage>,

    pub settings: OnceCell<gio::Settings>,

    #[template_child]
    toast_overlay: TemplateChild<adw::ToastOverlay>,

    #[template_child]
    progress_bar: TemplateChild<gtk::ProgressBar>,
    progress_bar_visible: Cell<bool>,

    pub songs: RefCell<Songs>,
}

impl Window {
    #[inline]
    pub fn init_settings_page(&self, style_manager: adw::StyleManager) {
        self.settings_page.init(
            style_manager,
            // Responsive background style widgets
            vec![
                self.sheet.get().upcast::<gtk::Widget>(),
                self.bottom_bar.get().upcast::<gtk::Widget>(),
            ],
            // Responsive menu style widgets
            vec![
                self.sheet_content.get().upcast::<gtk::Widget>(),
                (self.main_player.imp().media_controls.get()).upcast::<gtk::Widget>(),
            ],
            // Responsive OSD style widgets
            vec![
                (self.queue_page.imp().to_playing.get()).upcast::<gtk::Widget>(),
                (self.queue_page.imp().drag_row.get().unwrap().clone()).upcast::<gtk::Widget>(),
            ],
        );
    }

    #[allow(clippy::future_not_send)]
    pub async fn event_handler(&self, ui_rx: async_channel::Receiver<UpdateUI>) -> ! {
        let mut song_duration_ms = 0;
        loop {
            match ui_rx.recv().await.unwrap() {
                UpdateUI::SongInfo { item } => {
                    self.update_song_info(&item, &mut song_duration_ms);
                    let _ = mpris_tx().send(UpdateMPRIS::SongInfo(item)).await;
                }
                UpdateUI::PlayerTime { time } => {
                    self.main_player.set_time(time, song_duration_ms as f64);
                }
                UpdateUI::PlayerState {
                    playing,
                    interactive,
                } => {
                    self.main_player.set_state(playing, interactive);
                    let _ = mpris_tx().send(UpdateMPRIS::PlayState(playing)).await;
                }

                UpdateUI::SetQueue {
                    queue,
                    playing_index,
                } => self.set_song_queue(queue, playing_index),
                UpdateUI::SetQueueIndex(index) => self.set_queue_index(index),
                UpdateUI::RecenterQueue(index) => self.queue_page.recenter(index),
                UpdateUI::RedrawQueue => self.queue_page.redraw_queue(),
                UpdateUI::ExitQueueSelection => self.queue_page.exit_selection(),
                UpdateUI::OpenQueueSubpage(index) => self.open_queue_subpage(index),
                UpdateUI::ValidateQueueSubpageIndex => self.validate_subpage_index(),
                UpdateUI::CloseQueueSubpage => self.close_queue_subpage(),
                UpdateUI::Shuffle(shuffle) => self.queue_page.update_shuffle(shuffle),
                UpdateUI::Repeat(repeat) => self.queue_page.update_repeat(repeat),

                UpdateUI::Progress(progress) => self.update_progress(progress),
                UpdateUI::SetLibraryDirs(dirs) => self.set_library_dirs(&dirs),
                UpdateUI::SetLibrarySongs(songs) => self.load_library_songs(songs),
                UpdateUI::SetLibraryAlbums(albums) => self.load_library_albums(albums),
                UpdateUI::SetLibraryArtists(artists) => self.load_library_artists(artists),

                UpdateUI::LibrarySongLoaded { index, song } => self.song_loaded(index, song),
                UpdateUI::LibraryAlbumLoaded { index, song } => self.album_loaded(index, song),
                UpdateUI::QueueSongLoaded { index, song } => self.queue_song_loaded(index, song),
                UpdateUI::AlbumPageLoaded { index, song } => self.album_page_loaded(index, &song),

                UpdateUI::SongPageByIndex(index) => self.open_song_page_by_index(index),
                UpdateUI::SongPage(ctx) => self.open_song_page(ctx.0, ctx.1, ctx.2),
                UpdateUI::AlbumPage(album) => self.open_album_page(&album),
                UpdateUI::ArtistPage(artist) => self.open_artist_page(&artist),

                UpdateUI::FocusLibrary => self.focus_library(),
                UpdateUI::FocusPlaying => self.focus_playing(),
                UpdateUI::FocusSettings => self.focus_settings(),
                UpdateUI::OpenSheet(open) => self.open_sheet(open),
                UpdateUI::CanCloseSheet(can_close) => self.set_can_close_sheet(can_close),

                UpdateUI::RunAction(action) => {
                    WidgetExt::activate_action(&self.main_player.get(), action, None)
                        .expect(ACTION_ERR);
                }

                UpdateUI::Notification(message, action) => {
                    self.show_toast_notification(&message, action);
                }
                UpdateUI::DismissNotifications => self.toast_overlay.dismiss_all(),

                UpdateUI::Uninit => loop {
                    // Ignore any further requests without closing the channel
                    ui_rx.recv().await.unwrap();
                },

                UpdateUI::CrashNotice(error_info) => {
                    let dialog = gtk::AlertDialog::builder()
                        .modal(true)
                        .buttons(["Quit"])
                        .message("A component has crashed")
                        .detail(error_info)
                        .build();
                    let window = self.obj().to_owned();
                    dialog.choose(Some(&window.clone()), gio::Cancellable::NONE, move |_| {
                        WidgetExt::activate_action(&window, "app.quit", None).expect(ACTION_ERR);
                    });
                }
            }
        }
    }

    fn update_song_info(&self, item: &QueueItem, song_duration_ms: &mut u64) {
        #[cfg(feature = "startup-logs")]
        println!("update_song_info()");

        let song = match &item {
            QueueItem::Song(song) => song,
            QueueItem::Stopper(_) => {
                self.settings_page.reset_background_color();
                self.main_player.reset_info();
                return;
            }
        };

        let mut info = song.info();
        let (title, album, artist) = info.load_basic_and(|song_info| {
            *song_duration_ms = song_info.duration_ms;
            (
                song_info.title.clone(),
                song_info.album.clone(),
                song_info.artist.clone(),
            )
        });

        let detailed_info = info.try_inspect_detailed();
        let (artwork, has_artwork) = match detailed_info.as_deref() {
            Ok(Some(detailed)) => {
                self.lyrics_page.set_content(&title, &detailed.lyrics);
                (detailed.artwork.as_ref(), true)
            }
            _ => {
                let song = Arc::clone(song);
                let item = QueueItem::clone(item);
                Library::run_task(library_tx(), move || {
                    song.info().load_detailed();
                    let _ = ui_tx().send_blocking(UpdateUI::SongInfo { item });
                });
                (None, false)
            }
        };

        self.main_player
            .set_info(&title, &album, &artist, artwork, *song_duration_ms);
        drop(detailed_info);

        match &**info.load_thumbnail(UsedBy::SongQueue) {
            Some(thumbnail) => {
                self.settings_page.set_background_from_artwork(thumbnail);
                if !has_artwork {
                    self.main_player.set_artwork(Some(thumbnail));
                }
            }
            None => self.settings_page.reset_background_color(),
        }
    }

    fn set_song_queue(&self, queue: Box<[QueueItem]>, index: usize) {
        #[cfg(debug_assertions)]
        println!("set_song_queue(…, {index}): {} items", queue.len());

        self.queue_page.set_playing_index(index);
        self.queue_page.update_queue(queue, index);
    }
    fn set_queue_index(&self, index: usize) {
        #[cfg(debug_assertions)]
        println!("set_queue_index({index})");

        self.queue_page.set_playing_index(index);
        if self.queue_page.queue_length() != 0 {
            self.queue_page.redraw_queue();
        }
    }
    fn open_queue_subpage(&self, index: usize) {
        self.playing.push_by_tag("info");
        self.queue_subpage_visible.set(true);
        let queue = self.queue_page.borrow_queue();
        let stop_after = index + 1 < queue.len() && queue[index + 1].is_stopper();
        match &queue[index] {
            QueueItem::Song(song) => self.queue_subpage.show_song_info(index, Arc::clone(song)),
            QueueItem::Stopper(stopper) => self.queue_subpage.show_stopper_info(index, stopper),
        }
        self.queue_subpage.set_stop_after(stop_after);
    }
    fn validate_subpage_index(&self) {
        if !self.queue_subpage_visible.get() {
            return;
        }

        let item = self.queue_subpage.borrow_item().clone();
        let mut index = self.queue_subpage.index();
        let queue = self.queue_page.borrow_queue();

        {
            let current = &queue[index];
            if *current == item {
                self.queue_subpage.update_stop_after(index, &queue);
                return;
            }

            // If the song has moved to a new allocation, subpage should be reinitialized
            if let QueueItem::Song(current) = current
                && let QueueItem::Song(item) = &item
                && current.info().matches(&item.info())
            {
                self.queue_subpage
                    .show_song_info(index, Arc::clone(current));
                self.queue_subpage.update_stop_after(index, &queue);
                return;
            }
        }

        let (mut left, mut right) = (queue[..index].iter(), queue[index + 1..].iter());
        let mut offset = 0;
        loop {
            offset += 1;
            match (left.next_back(), right.next()) {
                (Some(left), _) if *left == item => break index -= offset,
                (_, Some(right)) if *right == item => break index += offset,
                (None, None) => return self.close_queue_subpage(),
                _ => (),
            }
        }

        self.queue_subpage.set_index(index);
        self.queue_subpage.update_stop_after(index, &queue);
    }
    fn close_queue_subpage(&self) {
        #[cfg(debug_assertions)]
        println!("close_queue_subpage()");
        while self.queue_subpage_visible.get() {
            self.playing.pop();
        }
    }
    fn update_progress(&self, progress: Option<f64>) {
        if let Some(progress) = progress {
            self.progress_bar.set_fraction(progress);
            if !self.progress_bar_visible.get() {
                self.progress_bar_visible.set(true);
                self.progress_bar.set_visible(true);
                self.library_page.switch_view("loading");
                self.settings_page.allow_library_refresh(false);
            }
        } else {
            self.progress_bar_visible.set(false);
            self.progress_bar.set_visible(false);
            self.library_page.switch_view("ready");
            self.settings_page.allow_library_refresh(true);

            // Update all open subpages so they display up-to-date information
            let song_pages = self.song_pages.borrow();
            let mut song_pages = song_pages.iter();
            let album_pages = self.album_pages.borrow();
            let mut album_pages = album_pages.iter();
            let mut album_page_index = 0; // Album pages need to know their index
            let artist_pages = self.artist_pages.borrow();
            let mut artist_pages = artist_pages.iter();
            for page_type in self.library_subpages.borrow().iter() {
                match page_type {
                    SubpageType::Song => {
                        if let Some(song_page) = song_pages.next() {
                            // FIX: It might be necessary to fully reinitialize the song
                            // page if the library context has changed, to avoid the play
                            // button playing the wrong item or crashing with OOB
                            song_page.refresh_ui();
                        }
                    }
                    SubpageType::Album => {
                        if let Some(album_page) = album_pages.next() {
                            album_page.refresh_ui(album_page_index);
                        }
                        album_page_index += 1;
                    }
                    SubpageType::Artist => {
                        if let Some(artist_page) = artist_pages.next() {
                            artist_page.refresh_ui();
                        }
                    }
                }
            }
            if self.queue_subpage_visible.get() {
                self.queue_subpage.refresh_ui();
            }
        }
    }

    fn set_library_dirs(&self, dirs: &[String]) {
        self.settings_page.set_directories(dirs);
    }

    pub fn focus_library(&self) {
        self.view_stack.set_visible_child_name("library");
    }
    pub fn focus_playing(&self) {
        self.view_stack.set_visible_child_name("playing");
    }
    pub fn focus_settings(&self) {
        self.view_stack.set_visible_child_name("settings");
    }
    pub fn open_sheet(&self, open: bool) {
        self.sheet.set_open(open);
    }
    pub fn set_can_close_sheet(&self, open: bool) {
        self.sheet.set_can_close(open);
    }
    pub fn toggle_sheet(&self) {
        self.sheet.set_open(!self.sheet.is_open());
    }

    // FIX: Slight stutter when the library songs/albums/artists are assigned
    fn load_library_songs(&self, songs: Songs) {
        self.library_page.set_empty(songs.is_empty());
        self.songs.replace(songs.clone());
        self.songs_page.load_songs(songs);
    }
    fn load_library_albums(&self, albums: Albums) {
        self.albums_page.load_albums(albums);
    }
    fn load_library_artists(&self, artists: Artists) {
        self.artists_page.load_artists(artists);
    }

    fn song_loaded(&self, index: usize, song: SharedSong) {
        let info = song.info();
        let Ok(thumbnail) = info.try_inspect_thumbnail() else {
            #[cfg(feature = "startup-logs")]
            println!("⚠️ {index}: library song thumbnail would block; retrying later...");

            Library::run_task(library_tx(), move || {
                thread::sleep(Duration::from_millis(30));
                let _ = ui_tx().send_blocking(UpdateUI::LibrarySongLoaded { index, song });
            });
            return;
        };
        if thumbnail.is_none() {
            return;
        }
        self.songs_page.assign_artwork(index, thumbnail.as_ref());
    }
    fn album_loaded(&self, index: usize, first_song: SharedSong) {
        let info = first_song.info();
        let Ok(thumbnail) = info.try_inspect_thumbnail() else {
            #[cfg(feature = "startup-logs")]
            println!("⚠️ {index}: library album thumbnail would block; retrying later...");

            Library::run_task(library_tx(), move || {
                thread::sleep(Duration::from_millis(30));
                let _ = ui_tx().send_blocking(UpdateUI::LibraryAlbumLoaded {
                    index,
                    song: first_song,
                });
            });
            return;
        };
        if thumbnail.is_none() {
            return;
        }
        self.albums_page.assign_artwork(index, thumbnail.as_ref());
    }
    fn queue_song_loaded(&self, index: usize, song: SharedSong) {
        let song_queue = self.queue_page.borrow_queue();
        if index >= song_queue.len() {
            return;
        }

        // Ensure the queue item hasn't changed before the message was handled
        // SAFETY: Function returns early if `index` is out of bounds
        let QueueItem::Song(cmp_song) = (unsafe { song_queue.get_unchecked(index) }) else {
            return;
        };
        if !Arc::ptr_eq(&song, cmp_song) {
            #[cfg(feature = "startup-logs")]
            println!("Queue thumbnail not set, because the queue item at {index} has changed");
            return cold_path();
        }

        let info = song.info();
        let Ok(thumbnail) = info.try_inspect_thumbnail() else {
            #[cfg(feature = "lock-warnings")]
            println!("{index}: queue song thumbnail would block; retrying later...");
            Library::run_task(library_tx(), move || {
                thread::sleep(Duration::from_millis(30));
                let _ = ui_tx().send_blocking(UpdateUI::QueueSongLoaded { index, song });
            });
            return;
        };
        if thumbnail.is_none() {
            return;
        }
        self.queue_page.assign_artwork(index, thumbnail.as_ref());
    }
    fn album_page_loaded(&self, index: usize, first_song: &SharedSong) {
        let album_pages = self.album_pages.borrow();
        if index >= album_pages.len() {
            return;
        }
        let info = first_song.info();
        let Some(ref detailed) = *info.inspect_detailed() else {
            return;
        };
        album_pages[index].assign_artwork(detailed.artwork.as_ref());
    }

    fn open_song_page_by_index(&self, index: usize) {
        let songs: Songs = self.songs.borrow().clone();
        self.open_song_page(index, Arc::clone(&songs[index]), Box::new(songs));
    }
    fn open_song_page(&self, index: usize, song: SharedSong, to_queue: Box<dyn ToQueue + Send>) {
        let song_page = SongPage::new(index, song, to_queue);
        self.library.push(&song_page);
        self.song_pages.borrow_mut().push(song_page);
        self.library_subpages.borrow_mut().push(SubpageType::Song);
    }
    fn open_album_page(&self, album: &SharedAlbum) {
        let _ = self.library.activate_action(
            "menu.album_page_play_mode",
            Some(&"Sequential".to_variant()),
        );
        let album_page = AlbumPage::new(album, self.album_pages.borrow().len());
        self.library.push(&album_page);
        self.album_pages.borrow_mut().push(album_page);
        self.library_subpages.borrow_mut().push(SubpageType::Album);
    }
    fn open_artist_page(&self, artist: &SharedArtist) {
        let _ = self.library.activate_action(
            "menu.artist_page_play_mode",
            Some(&"Sequential".to_variant()),
        );
        let artist_page = ArtistPage::new(artist);
        self.library.push(&artist_page);
        self.artist_pages.borrow_mut().push(artist_page);
        self.library_subpages.borrow_mut().push(SubpageType::Artist);
    }

    fn show_toast_notification(&self, message: &str, action: Option<ToastButtonAction>) {
        // TODO: Group same notifications
        let toast = adw::Toast::builder()
            .use_markup(false)
            .title(message)
            .build();
        if let Some(action) = action {
            // Dismiss previous notifications in case other undo buttons are currently
            // shown, because undoing in the wrong order can result in incorrect behavior
            self.toast_overlay.dismiss_all();

            toast.set_button_label(Some(action.0));
            toast.connect_button_clicked(move |_| action.1());
        }
        self.toast_overlay.add_toast(toast);
    }
}

#[glib::object_subclass]
impl ObjectSubclass for Window {
    const NAME: &str = "MellowWindow";
    type Type = super::Window;
    type ParentType = ApplicationWindow;

    fn class_init(class: &mut Self::Class) {
        class.bind_template();

        class.install_action_async("win.add_library", None, async |window, _, _| {
            let filter = gtk::FileFilter::new();
            filter.add_mime_type("inode/directory");
            let library_picker = gtk::FileDialog::builder()
                .modal(true)
                .default_filter(&filter)
                .accept_label("Add Library")
                .initial_folder(&gio::File::for_path(music_dir()))
                .build();

            if let Ok(dir) = library_picker.select_folder_future(Some(&window)).await {
                library_tx()
                    .send(LibraryRequest::AddLibrary(dir.path().unwrap()))
                    .expect(EXP_RX);
            } else {
                // Allow changing the library through the UI again if canceled,
                // otherwise it will re-activate when the directory list is updated
                window.imp().settings_page.allow_library_changes(true);
            }
        });

        class.install_action_async("win.queue_from_disk", None, async |window, _, _| {
            let filter = gtk::FileFilter::new();
            filter.add_mime_type("audio/*");
            filter.add_mime_type("inode/directory");
            let file_picker = gtk::FileDialog::builder()
                .modal(true)
                .default_filter(&filter)
                .accept_label("Play Now")
                .initial_folder(&gio::File::for_path(music_dir()))
                .build();

            // TODO: If possible, allow files _and_ folders
            if let Ok(dirs) = file_picker.open_multiple_future(Some(&window)).await {
                let mut paths = vec![];
                let mut index = 0;
                while let Some(path) = dirs.item(index) {
                    paths.push(path.downcast::<gio::File>().unwrap().path().unwrap());
                    index += 1;
                }
                library_tx()
                    .send(LibraryRequest::QueueFromPaths(paths))
                    .expect(EXP_RX);
            }
        });
    }

    fn instance_init(obj: &InitializingObject<Self>) {
        obj.init_template();
    }
}
impl ObjectImpl for Window {
    fn constructed(&self) {
        self.parent_constructed();

        self.main_player.init_seek();
        self.queue_page.init(self.queue_subpage.get());
        self.library.connect_popped(glib::clone!(
            #[weak(rename_to=window)]
            self,
            move |_, _| match window.library_subpages.borrow_mut().pop() {
                Some(SubpageType::Artist) => drop(window.artist_pages.borrow_mut().pop()),
                Some(SubpageType::Album) => drop(window.album_pages.borrow_mut().pop()),
                Some(SubpageType::Song) => drop(window.song_pages.borrow_mut().pop()),
                None => (),
            }
        ));
        self.playing.connect_popped(glib::clone!(
            #[weak(rename_to=window)]
            self,
            move |_, page| if page.downcast_ref::<QueueSubpage>().is_some() {
                window.queue_subpage_visible.set(false);
            },
        ));
    }
}
impl WindowImpl for Window {
    fn close_request(&self) -> glib::Propagation {
        self.obj().save_window_size().unwrap();
        self.obj().set_visible(false);
        match self.main_player.is_playing() && self.settings_page.play_in_background() {
            false => glib::Propagation::Proceed,
            true => glib::Propagation::Stop,
        }
    }
}
impl WidgetImpl for Window {
    fn size_allocate(&self, width: i32, height: i32, baseline: i32) {
        self.parent_size_allocate(width, height, baseline);
        self.main_player.update_spacing(height - 48);

        // IDEA: When the width is over 2/3rds of the height (and wider than N pixels),
        // use `width - height` of the width to show the lyrics on the side

        // IDEA: Redesign:
        // - Move the entire bottom sheet to a collapsible sidebar
        // - Add a lyrics tab instead of the settings, move settings to a popup window
        //   (but where would the settings be accessed from?)
        // - If sidebar is on the right, arrange tabs as such: Lyrics, Queue, Library
        //   (based on distance from and relevance to the main player)
        //
        // Benefits:
        // - Improves usability when using a wide window
        //
        // Downsides:
        // - Less separation between the main player and the overlay
        // - More buttons would need to be added to the main player
    }
}
impl ApplicationWindowImpl for Window {}
impl AdwApplicationWindowImpl for Window {}
