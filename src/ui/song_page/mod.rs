use adw::{prelude::*, subclass::prelude::*};
use glib::Object;
use gtk::glib;
use std::sync::Arc;

use crate::library::{SharedSong, ToQueue};

mod imp;

glib::wrapper! {
    pub struct SongPage(ObjectSubclass<imp::SongPage>)
        @extends adw::NavigationPage, gtk::Widget,
        @implements
            gtk::Accessible, gtk::Actionable, gtk::Buildable, gtk::Orientable, gtk::ConstraintTarget;
}

impl Default for SongPage {
    #[inline]
    fn default() -> Self {
        Object::builder().build()
    }
}

impl SongPage {
    /// Creates a new `SongPage` instance using the information from `song`,
    /// using the `index` and `to_queue` arguments for the play button behavior
    #[inline]
    #[must_use]
    pub fn new(index: usize, song: SharedSong, to_queue: Box<dyn ToQueue + Send>) -> SongPage {
        let song_page = Self::default();
        song_page.init_page(index, song, to_queue);
        song_page
    }

    /// Initializes the song page using the provided arguments
    ///
    /// # Panics
    /// The function panics if any of the `song`'s `RwLock` is in a poisoned state
    #[inline]
    pub fn init_page(&self, index: usize, song: SharedSong, to_queue: Box<dyn ToQueue + Send>) {
        let ui = self.imp();

        ui.index.set(index);
        ui.shared_song.replace(Some(Arc::clone(&song)));

        let mut info = song.info();
        info.load_basic_and(|song_info| {
            self.set_title(&["Song: ", &song_info.title].concat());
            ui.song_title.set_label(&song_info.title);
            ui.album_title.set_label(&song_info.album);
            ui.artist_name.set_label(&song_info.artist);
            ui.context.replace(Some(to_queue));
        });

        ui.rating.set_rating_silent(info.user().rating);
        ui.rating.connect_rating_set(move |rating| {
            song.info().set_rating(rating);
        });
    }

    /// Refreshes the song page by reinitializing it
    ///
    /// # Panics
    /// Panics if the page was not initialized
    pub fn refresh_ui(&self) {
        let ui = self.imp();
        // FIX: The `context` and `index` might no longer be correct if the relevant
        // context has changed due to songs being added to or removed from the library
        self.init_page(
            ui.index.get(),
            // FIX: What should be done if the song was removed from the library
            // while its page is still open?
            ui.shared_song.take().unwrap(),
            ui.context.take().unwrap(),
        );
    }
}
