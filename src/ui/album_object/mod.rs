use adw::subclass::prelude::*;
use core::{cmp, sync::atomic};
use glib::{Object, object::ObjectExt};
use gtk::{gdk, glib};
use std::sync::Arc;

use crate::library::{Library, SharedAlbum, library_tx};
use crate::ui::{SortConfig, UpdateUI, ui_tx};

mod imp;

glib::wrapper! {
    /// # Safety
    /// Either construct using `AlbumObject::new()`, or ensure
    /// that `….imp().first_song` is initialized if constructing
    /// manually. Failing to do so will lead to undefined behavior.
    pub struct AlbumObject(ObjectSubclass<imp::AlbumObject>);
}

impl AlbumObject {
    #[inline]
    #[must_use]
    pub fn new(
        index: u32,
        album: &str,
        artist: &str,
        year: u32,
        shared_album: SharedAlbum,
    ) -> Self {
        let album_object: AlbumObject = Object::builder()
            .property("index", index)
            .property("album", album)
            .property("artist", artist)
            .property("year", year)
            .build();
        let _ = album_object.imp().shared_album.set(shared_album);
        album_object
    }

    /// Loads the artwork thumbnail in a background thread
    ///
    /// # Panics
    /// The function panics if the album `Mutex` is poisoned
    #[inline]
    pub fn load_artwork(&self) {
        if self.artwork().is_some() {
            return;
        }
        let index = self.index() as usize;
        let imp = self.imp();
        let album = Arc::clone(self.shared_album());
        let is_visible = Arc::clone(&imp.is_visible);
        is_visible.store(true, atomic::Ordering::Relaxed);
        Library::run_task(library_tx(), move || {
            if !is_visible.load(atomic::Ordering::Relaxed) {
                return;
            }
            let song = Arc::clone(album.lock().unwrap().first_song());
            drop(song.info().load_thumbnail());
            song.info().unload_detailed(); // `load_thumbnail` may have loaded it
            let _ = ui_tx().send(UpdateUI::LibraryAlbumLoaded { index, song });
        });
    }

    /// Unloads the artwork thumbnail in a background thread
    ///
    /// # Panics
    /// The function panics if the album `Mutex` is poisoned
    #[inline]
    pub fn unload_artwork(&self) {
        self.set_property("artwork", Option::<gdk::Texture>::None);
        let imp = self.imp();
        let album = Arc::clone(self.shared_album());
        let is_visible = Arc::clone(&imp.is_visible);
        is_visible.store(false, atomic::Ordering::Relaxed);
        // NOTE: Unloading in the background in case the `RwLock` is busy
        Library::run_task(library_tx(), move || {
            if is_visible.load(atomic::Ordering::Relaxed) {
                return;
            }
            album.lock().unwrap().first_song().info().unload_thumbnail();
        });
    }

    /// Returns the `SharedAlbum` associated with this object
    #[inline]
    #[must_use]
    pub fn shared_album(&self) -> &SharedAlbum {
        self.imp().shared_album()
    }

    /// Returns the ordering of `self` compared to `other`,
    /// based on the sort mode specified using `order_by`
    #[inline]
    #[must_use]
    pub fn order_cmp(&self, other: &Self, order_by: SortConfig<AlbumOrdering>) -> gtk::Ordering {
        let ord = match other.rank().total_cmp(&self.rank()) {
            cmp::Ordering::Equal => match order_by.ordering.get() {
                AlbumOrdering::Default => self.cmp_artist_year_album(other),
                AlbumOrdering::ReleaseDate => self.cmp_release_date(other),
                AlbumOrdering::Modified => self.cmp_modified_newer(other),
                AlbumOrdering::Added => self.cmp_added_newer(other),
                AlbumOrdering::PlayCount => self.cmp_most_played(other),
                AlbumOrdering::Rating => self.cmp_best_rating(other),
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
    fn cmp_artist_year_album(&self, other: &Self) -> cmp::Ordering {
        match self.artist().cmp(&other.artist()) {
            cmp::Ordering::Equal => match self.year().cmp(&other.year()) {
                cmp::Ordering::Equal => self.album().cmp(&other.album()),
                ordering => ordering,
            },
            ordering => ordering,
        }
    }
    #[inline]
    #[must_use]
    fn cmp_most_played(&self, other: &Self) -> cmp::Ordering {
        match other.played().total_cmp(&self.played()) {
            cmp::Ordering::Equal => self.index().cmp(&other.index()),
            ordering => ordering,
        }
    }
    #[inline]
    #[must_use]
    fn cmp_best_rating(&self, other: &Self) -> cmp::Ordering {
        match other.rating().total_cmp(&self.rating()) {
            cmp::Ordering::Equal => self.cmp_most_played(other),
            ordering => ordering,
        }
    }
    #[inline]
    #[must_use]
    fn cmp_release_date(&self, other: &Self) -> cmp::Ordering {
        match other.year().cmp(&self.year()) {
            cmp::Ordering::Equal => self.index().cmp(&other.index()),
            ordering => ordering,
        }
    }
    #[inline]
    #[must_use]
    fn cmp_modified_newer(&self, other: &Self) -> cmp::Ordering {
        // NOTE: Comparing modification time using the first song is not necessarily correct
        match other.modified().cmp(&self.modified()) {
            cmp::Ordering::Equal => self.cmp_artist_year_album(other),
            ordering => ordering,
        }
    }
    #[inline]
    #[must_use]
    fn cmp_added_newer(&self, other: &Self) -> cmp::Ordering {
        match other.added().cmp(&self.added()) {
            cmp::Ordering::Equal => self.cmp_artist_year_album(other),
            ordering => ordering,
        }
    }
}

#[derive(Default)]
pub struct AlbumData {
    index: u32,
    album: String,
    artist: String,
    artwork: Option<gdk::Texture>,
    year: u32,
    rank: f64,
    rating: f64,
    played: f64,
    modified: i64,
    added: u64,
}

#[derive(Clone, Copy)]
pub enum AlbumOrdering {
    Default,
    ReleaseDate,
    Modified,
    Added,
    Rating,
    PlayCount,
}

impl AlbumOrdering {
    #[inline]
    #[must_use]
    pub const fn to_str(self) -> &'static str {
        match self {
            AlbumOrdering::Default => "Default",
            AlbumOrdering::Rating => "Rating",
            AlbumOrdering::PlayCount => "Play Count",
            AlbumOrdering::ReleaseDate => "Release Date",
            AlbumOrdering::Added => "Added",
            AlbumOrdering::Modified => "Modified",
        }
    }
}
impl From<&str> for AlbumOrdering {
    #[inline]
    fn from(value: &str) -> Self {
        match value {
            "Default" => AlbumOrdering::Default,
            "Rating" => AlbumOrdering::Rating,
            "Play Count" => AlbumOrdering::PlayCount,
            "Release Date" => AlbumOrdering::ReleaseDate,
            "Added" => AlbumOrdering::Added,
            "Modified" => AlbumOrdering::Modified,
            _ => unimplemented!(),
        }
    }
}
