use adw::{prelude::*, subclass::prelude::*};
use core::{cmp, sync::atomic};
use glib::Object;
use gtk::{gdk, glib};
use std::sync::Arc;

use crate::library::{Library, SharedSong, library_tx};
use crate::ui::{SortConfig, UpdateUI, ui_tx};

mod imp;

glib::wrapper! {
    /// # Safety
    /// Either construct using `SongObject::new()`, or ensure
    /// that `….imp().shared_song` is initialized if constructing
    /// manually. Failing to do so will lead to undefined behavior.
    pub struct SongObject(ObjectSubclass<imp::SongObject>);
}

impl SongObject {
    #[inline]
    #[must_use]
    pub fn new(index: u32, song: SharedSong) -> Self {
        let (title, album, artist, year) = song.info().load_basic_and(|info| {
            (
                info.title.clone(),
                info.album.clone(),
                info.artist.clone(),
                info.year as u32,
            )
        });
        let song_object: SongObject = Object::builder()
            .property("index", index)
            .property("song", title)
            .property("album", album)
            .property("artist", artist)
            .property("year", year)
            .build();
        let _ = song_object.imp().shared_song.set(song);
        song_object
    }

    /// Loads the artwork thumbnail in a background thread
    #[inline]
    pub fn load_artwork(&self) {
        if self.artwork().is_some() {
            return;
        }
        let imp = self.imp();
        let index = self.index() as usize;
        let song = Arc::clone(imp.shared_song());
        let is_visible = Arc::clone(&imp.is_visible);
        is_visible.store(true, atomic::Ordering::Release);
        Library::run_task(library_tx(), move || {
            if !is_visible.load(atomic::Ordering::Acquire) {
                return;
            }
            drop(song.info().load_thumbnail());
            song.info().unload_detailed(); // `load_thumbnail` may have loaded it
            let _ = ui_tx().send(UpdateUI::LibrarySongLoaded { index, song });
        });
    }

    /// Unloads the artwork thumbnail in a background thread
    #[inline]
    pub fn unload_artwork(&self) {
        self.set_property("artwork", Option::<gdk::Texture>::None);
        let imp = self.imp();
        let song = Arc::clone(imp.shared_song());
        let is_visible = Arc::clone(&imp.is_visible);
        is_visible.store(false, atomic::Ordering::Release);
        // NOTE: Unloading in the background in case the `RwLock` is busy
        Library::run_task(library_tx(), move || {
            if is_visible.load(atomic::Ordering::Acquire) {
                return;
            }
            song.info().unload_thumbnail();
        });
    }

    /// Returns the `SharedSong` associated with this object
    #[inline]
    #[must_use]
    pub fn shared_song(&self) -> SharedSong {
        Arc::clone(self.imp().shared_song())
    }

    /// Returns the ordering of `self` compared to `other`,
    /// based on the sort mode specified using `order_by`
    #[inline]
    #[must_use]
    pub fn order_cmp(&self, other: &Self, order_by: SortConfig<SongOrdering>) -> gtk::Ordering {
        let ord = match other.rank().total_cmp(&self.rank()) {
            cmp::Ordering::Equal => match order_by.ordering.get() {
                SongOrdering::Default => self.cmp_default(other),
                SongOrdering::Rating => self.cmp_best_rating(other),
                SongOrdering::PlayCount => self.cmp_most_played(other),
                SongOrdering::ReleaseDate => self.cmp_release_date(other),
                SongOrdering::Added => self.cmp_added_newer(other),
                SongOrdering::Modified => self.cmp_modified_newer(other),
                SongOrdering::Random => self.cmp_random(other),
            },
            ordering => ordering,
        };
        if order_by.reversed.get() {
            return ord.reverse().into();
        }
        ord.into()
    }
    #[inline]
    #[must_use]
    fn cmp_default(&self, other: &Self) -> cmp::Ordering {
        match self.artist().cmp(&other.artist()) {
            cmp::Ordering::Equal => self.index().cmp(&other.index()),
            ordering => ordering,
        }
    }
    #[inline]
    #[must_use]
    fn cmp_best_rating(&self, other: &Self) -> cmp::Ordering {
        match other.rating().cmp(&self.rating()) {
            cmp::Ordering::Equal => self.cmp_most_played(other),
            ordering => ordering,
        }
    }
    #[inline]
    #[must_use]
    fn cmp_most_played(&self, other: &Self) -> cmp::Ordering {
        match other.played().cmp(&self.played()) {
            cmp::Ordering::Equal => self.cmp_default(other),
            ordering => ordering,
        }
    }
    #[inline]
    #[must_use]
    fn cmp_release_date(&self, other: &Self) -> cmp::Ordering {
        match other.year().cmp(&self.year()) {
            cmp::Ordering::Equal => self.cmp_default(other),
            ordering => ordering,
        }
    }
    #[inline]
    #[must_use]
    fn cmp_added_newer(&self, other: &Self) -> cmp::Ordering {
        match other.modified().cmp(&self.modified()) {
            cmp::Ordering::Equal => self.cmp_default(other),
            ordering => ordering,
        }
    }
    #[inline]
    #[must_use]
    fn cmp_modified_newer(&self, other: &Self) -> cmp::Ordering {
        match other.modified().cmp(&self.modified()) {
            cmp::Ordering::Equal => self.cmp_default(other),
            ordering => ordering,
        }
    }
    #[inline]
    #[must_use]
    fn cmp_random(&self, other: &Self) -> cmp::Ordering {
        other.random().cmp(&self.random())
    }
}

#[derive(Default)]
pub struct SongData {
    index: u32,
    song: String,
    album: String,
    artist: String,
    artwork: Option<gdk::Texture>,
    year: u32,
    rank: f64,
    rating: u8,
    played: u64,
    modified: u64,
    added: u64,
    random: u64,
}

#[derive(Clone, Copy)]
pub enum SongOrdering {
    Default,
    Rating,
    PlayCount,
    ReleaseDate,
    Added,
    Modified,
    Random,
}

impl SongOrdering {
    #[inline]
    #[must_use]
    pub const fn to_str(self) -> &'static str {
        match self {
            SongOrdering::Default => "Default",
            SongOrdering::Rating => "Rating",
            SongOrdering::PlayCount => "Play Count",
            SongOrdering::ReleaseDate => "Release Date",
            SongOrdering::Added => "Added",
            SongOrdering::Modified => "Modified",
            SongOrdering::Random => "Random",
        }
    }
}
impl From<&str> for SongOrdering {
    #[inline]
    fn from(value: &str) -> Self {
        match value {
            "Default" => SongOrdering::Default,
            "Rating" => SongOrdering::Rating,
            "Play Count" => SongOrdering::PlayCount,
            "Release Date" => SongOrdering::ReleaseDate,
            "Added" => SongOrdering::Added,
            "Modified" => SongOrdering::Modified,
            _ => unimplemented!(),
        }
    }
}
