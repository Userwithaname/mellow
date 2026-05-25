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
    ///
    /// # Panics
    /// The function panics if any of the `song`'s `RwLock` is in a poisoned state
    #[inline]
    #[must_use]
    pub fn new(index: usize, song: SharedSong, to_queue: Box<dyn ToQueue + Send>) -> SongPage {
        let song_page = Self::default();
        let ui = song_page.imp();

        ui.index.set(index);
        let mut info = song.info();

        info.load_basic_and(|song_info| {
            song_page.set_title(&["Song: ", &song_info.title].concat());
            ui.song_title.set_label(&song_info.title);
            ui.album_title.set_label(&song_info.album);
            ui.artist_name.set_label(&song_info.artist);
            ui.context.replace(Some(to_queue));
        });

        let user_info = info.user();
        ui.rating.set_rating_silent(user_info.rating);
        drop(user_info);
        drop(info);

        ui.shared_song.replace(Some(Arc::clone(&song)));
        ui.rating.connect_rating_set(move |rating| {
            song.info().set_rating(rating);
        });

        song_page
    }
}
