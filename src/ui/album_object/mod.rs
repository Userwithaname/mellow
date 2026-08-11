use adw::subclass::prelude::*;
use core::{cmp, sync::atomic};
use glib::{Object, object::ObjectExt};
use gtk::{gdk, glib};
use std::sync::Arc;

use crate::library::{Library, SharedAlbum, library_tx};
use crate::ui::{LibraryObject, LibrarySort, Sortable, UpdateUI, ui_tx};

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
        is_visible.store(true, atomic::Ordering::Release);
        Library::run_task(library_tx(), move || {
            if !is_visible.load(atomic::Ordering::Acquire) {
                return;
            }
            let song = Arc::clone(album.lock().unwrap().first_song());
            drop(song.info().load_thumbnail());
            song.info().unload_detailed(); // `load_thumbnail` may have loaded it
            let _ = ui_tx().send_blocking(UpdateUI::LibraryAlbumLoaded { index, song });
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
        is_visible.store(false, atomic::Ordering::Release);
        // NOTE: Unloading in the background in case the `RwLock` is busy
        Library::run_task(library_tx(), move || {
            if is_visible.load(atomic::Ordering::Acquire) {
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
    pub fn order_cmp(&self, other: &Self, order_by: &LibrarySort) -> gtk::Ordering {
        (other.rank().total_cmp(&self.rank()))
            .then_with(|| order_by.cmp(self, other))
            .into()
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
    /// Rating, as displayed in the UI (0 if unassigned)
    stars: f64,
    /// Rating with a fallback value (3 if unassigned, used for sorting)
    rating: f64,
    played: f64,
    modified: u64,
    added: u64,
    random: u64,
    tags: Vec<String>,
}

impl LibraryObject for AlbumObject {
    #[inline]
    fn play_count(&self) -> f64 {
        self.played()
    }
    #[inline]
    fn stars(&self) -> f64 {
        self.stars()
    }
    #[inline]
    fn rating(&self) -> f64 {
        self.rating()
    }
    #[inline]
    fn year(&self) -> u32 {
        self.year()
    }
    #[inline]
    fn modified(&self) -> u64 {
        self.modified()
    }
    #[inline]
    fn added(&self) -> u64 {
        self.added()
    }
    #[inline]
    fn tags(&self) -> Vec<String> {
        self.tags()
    }
}

impl Sortable for AlbumObject {
    #[inline]
    fn sort_default(&self, other: &Self) -> cmp::Ordering {
        (self.artist().cmp(&other.artist()))
            .then_with(|| self.year().cmp(&other.year()))
            .then_with(|| self.album().cmp(&other.album()))
    }
    #[inline]
    fn sort_random(&self, other: &Self) -> cmp::Ordering {
        self.random().cmp(&other.random())
    }
}
