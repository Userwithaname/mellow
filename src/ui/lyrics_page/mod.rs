use adw::subclass::prelude::*;
use gtk::glib;

use crate::library::lyrics::{Lyrics, SyncedLyric};

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

        match lyrics {
            Lyrics::Unsynced(lyrics) if !lyrics.is_empty() => {
                lyrics_page.lyrics.set_label(lyrics);
            }
            Lyrics::Synced(synced_lyrics) => {
                // TODO: Highlight the current synced lyric line
                // The `imp` module could hold a copy of the `Lyrics`, and remember the next
                // `SyncedLyric`; when time exceeds the lyric time, the next lyric and the UI
                // would update. This would be done inside a new `update` function, whech would
                // be called when processing `UpdateUI::PlayerTime`. A special case would be
                // needed for seeking, which would locate the correct lyric line.
                // Highlighting could be done using CSS styles if each line has its own individual
                // label (within `GtkListView` or `GtkListBox`).

                // The below is only a temporary solution until proper synced lyrics implementation
                let lyrics = synced_lyrics
                    .iter()
                    .map(|SyncedLyric { lyric, .. }| lyric.to_owned() + "\n")
                    .collect::<String>();
                lyrics_page.lyrics.set_label(&lyrics);
            }
            // TODO: Support translations (see `gettext-rs`)
            Lyrics::Unsynced(_) => lyrics_page.lyrics.set_label("Lyrics not available"),
        }
    }
}
