use adw::subclass::prelude::*;
use gtk::glib;

use crate::library::lyrics::Lyrics;

mod imp;

glib::wrapper! {
    pub struct LyricsPage(ObjectSubclass<imp::LyricsPage>)
        @extends adw::NavigationPage, gtk::Widget,
        @implements
            gtk::Accessible, gtk::Actionable, gtk::Buildable, gtk::Orientable, gtk::ConstraintTarget;
}

impl LyricsPage {
    pub fn set_content(&self, song_title: &str, lyrics: &Lyrics) {
        let lyrics_page = self.imp();
        lyrics_page.song_title.set_label(song_title);
        lyrics_page.load_lyrics(lyrics);
    }
    pub fn update_synced_lyrics(&self, time_ms: u64) {
        self.imp().update_synced_lyrics(time_ms);
    }
}
