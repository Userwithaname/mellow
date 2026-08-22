use adw::{prelude::*, subclass::prelude::*};
use glib::{Object, clone};
use gtk::{Orientation, gdk, glib};
use std::sync::{Arc, atomic::Ordering};

use crate::excuses::EXP_RX;
use crate::library::unload_unused::UsedBy;
use crate::library::{Library, SharedAlbum, library_tx};
use crate::ui::{ListRow, UpdateUI, fallback_album_image, show_queue, ui_tx};
use crate::util::{format_duration_minutes, format_duration_ms};

mod imp;

glib::wrapper! {
    pub struct AlbumPage(ObjectSubclass<imp::AlbumPage>)
        @extends adw::NavigationPage, gtk::Widget,
        @implements
            gtk::Accessible, gtk::Actionable, gtk::Buildable, gtk::Orientable, gtk::ConstraintTarget;
}

impl Default for AlbumPage {
    #[inline]
    fn default() -> Self {
        Object::builder().build()
    }
}

impl AlbumPage {
    /// Creates a new `AlbumPage` instance using the information from `album`
    #[inline]
    #[must_use]
    pub fn new(album: &SharedAlbum, page_index: usize) -> AlbumPage {
        let album_page = Self::default();
        album_page.init_page(album, page_index);
        album_page
    }

    /// Initializes the album page using the information from `album`
    ///
    /// # Panics
    /// The function panics if any of the `album`'s `Mutex`es or the `album.songs`'
    /// `RwLock`s are in a poisoned state. It may also panic at runtime upon
    /// interaction if the UI channel is closed.
    #[inline]
    pub fn init_page(&self, album: &SharedAlbum, page_index: usize) {
        let ui = self.imp();

        ui.album.replace(Some(Arc::clone(album)));
        ui.rating.set_item(Box::new(Arc::clone(album)));

        let album_locked = album.lock().unwrap();
        self.set_title(&["Album: ", album_locked.title()].concat());
        ui.album_title.set_label(album_locked.title());
        ui.artist_name
            .set_label(album_locked.artist().lock().unwrap().name());
        match album_locked.year() {
            year if year > 0 => ui.year.set_label(&year.to_string()),
            _ => ui.year.set_visible(false),
        }

        // NOTE: The below values must to be manually updated when changing the .ui file
        let mut album_group_index = 1_i32; // The index at which new groups are inserted
        let default_group_count = 2; // Number of groups of an empty page (counting from 1)

        // When updating an existing page, previously added groups need to be removed
        while ui.album_pref_page.group(default_group_count).is_some() {
            ui.album_pref_page
                .remove(&ui.album_pref_page.group(album_group_index as u32).unwrap());
        }

        let mut disc_number = !0;
        let mut duration_total_ms = 0;
        let mut album_group = adw::PreferencesGroup::new();

        for (i, song) in album_locked.songs().iter().enumerate() {
            let song_row = ListRow::new();

            song.info().load_basic_and(|info| {
                song_row.add_prefix(
                    &gtk::Label::builder()
                        .width_chars(2)
                        .label(info.track.to_string())
                        .justify(gtk::Justification::Center)
                        .css_classes(["dimmed", "numeric"])
                        .build(),
                );
                song_row.set_title(&info.title);
                let duration = info.duration_ms;
                song_row.set_suffix_label(&format_duration_ms(duration));
                duration_total_ms += duration;

                let song = Arc::clone(song);
                let album = Arc::clone(album);
                song_row.connect_activated(move |_| {
                    ui_tx()
                        .send_blocking(UpdateUI::SongPage(Box::new((
                            i,
                            Arc::clone(&song),
                            Box::new(Arc::clone(&album)),
                        ))))
                        .expect(EXP_RX);
                });

                ui.details
                    .set_label(&format_duration_minutes(duration_total_ms / (1000 * 60)));

                if info.disc != disc_number {
                    disc_number = info.disc;
                    let play_buttons = gtk::Box::new(Orientation::Horizontal, 16);
                    let queue_disc_button = gtk::Button::builder()
                        // TODO: Support translations
                        .tooltip_text(format!("Add Disc {disc_number} To Queue"))
                        .icon_name("list-add-symbolic")
                        .css_name("flat")
                        .build();
                    queue_disc_button.connect_clicked(clone!(
                        #[weak(rename_to=album_page)]
                        ui,
                        move |_| album_page.add_disc_to_queue(disc_number)
                    ));
                    queue_disc_button.set_cursor_from_name(Some("pointer"));
                    let play_disc_button = gtk::Button::builder()
                        // TODO: Support translations
                        .tooltip_text(format!("Play Disc {disc_number}"))
                        .icon_name("media-playback-start-symbolic")
                        .css_name("flat")
                        .build();
                    play_disc_button.connect_clicked(clone!(
                        #[weak(rename_to=album_page)]
                        ui,
                        move |_| album_page.play_disc(disc_number)
                    ));
                    play_disc_button.set_cursor_from_name(Some("pointer"));
                    play_buttons.append(&queue_disc_button);
                    play_buttons.append(&play_disc_button);
                    album_group = adw::PreferencesGroup::builder()
                        // TODO: Support translations
                        .title(format!("Disc {disc_number}"))
                        .header_suffix(&play_buttons)
                        .build();
                    ui.album_pref_page.insert(&album_group, album_group_index);
                    album_group_index += 1;
                }
            });

            album_group.add(&song_row);
        }

        let first_song = Arc::clone(album_locked.first_song());
        let mut info = first_song.info();
        let Some(ref detailed_info) = *info.inspect_detailed() else {
            match info.load_thumbnail(UsedBy::None).as_ref() {
                None => ui.album_cover.set_paintable(Some(&fallback_album_image())),
                thumbnail => ui.album_cover.set_paintable(thumbnail),
            }

            let cancel = Arc::clone(&ui.cancel_artowrk_loading);
            Library::run_task(library_tx(), move || {
                if cancel.load(Ordering::Relaxed) {
                    #[cfg(feature = "verbose-logs")]
                    println!("Arwork loading cancelled");
                    return;
                }
                first_song.info().load_detailed();
                if cancel.load(Ordering::Relaxed) {
                    #[cfg(feature = "verbose-logs")]
                    println!("Arwork assignment cancelled");
                    return;
                }
                let _ = ui_tx().send_blocking(UpdateUI::AlbumPageLoaded {
                    index: page_index,
                    song: first_song,
                });
            });

            return;
        };

        match detailed_info.artwork.as_ref() {
            None => ui.album_cover.set_paintable(Some(&fallback_album_image())),
            artwork => ui.album_cover.set_paintable(artwork),
        }
    }

    /// Refreshes the artist page by reinitializing it
    ///
    /// # Panics
    /// Panics if the page was not initialized
    pub fn refresh_ui(&self, page_index: usize) {
        // NOTE: Removing the album from the library while its page is open will
        // still allow it to be played, which will cause a playback error if the
        // actual files were removed (should this be handled?)
        // NOTE: Borrowing `album` directly would panic on re-borrow
        self.init_page(&self.imp().album.take().unwrap(), page_index);
    }

    /// Assigns the album artwork to be shown on the page
    #[inline]
    pub fn assign_artwork(&self, artwork: Option<&gdk::Texture>) {
        if artwork.is_some() {
            self.imp().album_cover.set_paintable(artwork);
        } else {
            self.imp()
                .album_cover
                .set_paintable(Some(&fallback_album_image()));
        }
    }

    /// Sets the shuffle mode for the play button
    #[inline]
    pub fn set_shuffle(&self, shuffle: bool) {
        self.imp().set_shuffle(shuffle);
    }

    /// Starts a new queue with all songs from the currently shown album
    #[inline]
    pub fn play_now(&self, shuffle: bool) {
        imp::AlbumPage::play_now(self.imp().all_songs(), shuffle);
    }
    /// Adds all songs from the currently shown album to the player queue
    #[inline]
    pub fn add_to_queue(&self) {
        let ui_tx = ui_tx();
        let _ = ui_tx.send_blocking(UpdateUI::RunAction("ui.library_nav_pop"));
        let imp = self.imp();
        imp::AlbumPage::add_to_queue(imp.all_songs());
        let _ = ui_tx.send_blocking(UpdateUI::Notification(
            format!(
                "Album \"{}\" has been added to queue",
                imp.album_title.label()
            ),
            Some(Box::new(("View", Box::new(show_queue)))),
        ));
    }
}
