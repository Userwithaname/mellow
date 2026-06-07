use adw::{prelude::*, subclass::prelude::*};
use glib::Object;
use gtk::glib;
use std::sync::Arc;

use crate::excuses::EXP_RX;
use crate::library::SharedArtist;
use crate::ui::ListRow;
use crate::ui::{UpdateUI, fallback_album_image, ui_tx};

mod imp;

glib::wrapper! {
    pub struct ArtistPage(ObjectSubclass<imp::ArtistPage>)
        @extends adw::NavigationPage, gtk::Widget,
        @implements
            gtk::Accessible, gtk::Actionable, gtk::Buildable, gtk::Orientable, gtk::ConstraintTarget;
}

impl Default for ArtistPage {
    #[inline]
    fn default() -> Self {
        Object::builder().build()
    }
}

impl ArtistPage {
    /// Creates a new `ArtistPage` instance using the information from `artist`
    #[inline]
    #[must_use]
    pub fn new(artist: &SharedArtist) -> ArtistPage {
        let artist_page = Self::default();
        artist_page.init_page(artist);
        artist_page
    }

    /// Initializes the artist page using the information from `artist`
    ///
    /// # Panics
    /// The function panics if any of the artist albums' `Mutex`es or songs'
    /// `RwLock`s are in a poisoned state. It may also panic at runtime upon
    /// interaction if the UI channel is closed.
    #[inline]
    pub fn init_page(&self, artist: &SharedArtist) {
        let ui = self.imp();

        ui.artist.replace(Some(Arc::clone(artist)));

        let artist = artist.lock().unwrap();
        let albums = artist.albums();
        self.set_title(&["Artist: ", artist.name()].concat());
        ui.artist_name.set_label(artist.name());
        ui.album_count.set_label(
            &format!("{} Albums", albums.len()), //TODO: Translations & grammar
        );

        ui.albums_list.remove_all();

        let fallback_image = fallback_album_image();
        for album in albums {
            let album_row = ListRow::new();

            let album_locked = album.lock().unwrap();
            album_row.set_title(album_locked.title());
            album_row.set_subtitle(&match album_locked.year() {
                year if year > 0 => year.to_string(),
                _ => String::new(),
            });

            let mut info = album_locked.first_song().info();
            match info.load_thumbnail().as_ref() {
                None => album_row.set_prefix_image(Some(&fallback_image)),
                thumbnail => album_row.set_prefix_image(thumbnail),
            }

            drop(album_locked);
            let album = Arc::clone(album);
            album_row.connect_activated(move |_| {
                (ui_tx().send(UpdateUI::AlbumPage(Arc::clone(&album)))).expect(EXP_RX);
            });

            ui.albums_list.append(&album_row);
        }
    }

    /// Refreshes the artist page by reinitializing it
    ///
    /// # Panics
    /// Panics if the page was not initialized
    pub fn refresh_ui(&self) {
        // FIX: What should be done if the artist was removed from the library
        // while its page is still open?
        // NOTE: Borrowing `artist` directly would panic on re-borrow
        self.init_page(&self.imp().artist.take().unwrap());
    }

    /// Sets the shuffle mode for the play button
    #[inline]
    pub fn set_shuffle(&self, shuffle: bool) {
        self.imp().set_shuffle(shuffle);
    }
}
