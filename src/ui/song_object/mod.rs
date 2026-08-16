use adw::{prelude::*, subclass::prelude::*};
use core::sync::atomic::{AtomicBool, Ordering};
use glib::Object;
use gtk::{gdk, glib};
use std::sync::Arc;

use crate::cold_expression;
use crate::library::unload_unused::UsedBy;
use crate::library::{Library, SharedSong, library_tx};
use crate::ui::{LibraryObject, LibrarySort, Sortable, UpdateUI, ui_tx};

mod imp;

glib::wrapper! {
    /// # Safety
    /// Either construct using `SongObject::new()`, or ensure
    /// that `….imp().shared_song` is initialized if constructing
    /// manually. Failing to do so will lead to undefined behavior.
    pub struct SongObject(ObjectSubclass<imp::SongObject>);
}

pub struct InfoNotLoadedError;

impl SongObject {
    /// Constructs a new `SongObject`
    ///
    /// # Errors
    /// Returns an error if the song info is not loaded
    #[inline]
    pub fn new(index: u32, song: SharedSong) -> Result<Self, InfoNotLoadedError> {
        let (title, album, artist, year) = match &*song.info().inspect_basic() {
            Some(info) => (
                info.title.clone(),
                info.album.clone(),
                info.artist.clone(),
                info.year as u32,
            ),
            None => cold_expression! { return Err(InfoNotLoadedError) },
        };

        let song_object: SongObject = Object::builder()
            .property("index", index)
            .property("song", title)
            .property("album", album)
            .property("artist", artist)
            .property("year", year)
            .build();
        let _ = song_object.imp().shared_song.set(song);

        Ok(song_object)
    }

    /// Loads the artwork thumbnail in a background thread
    ///
    /// The function also marks the item as visible, so it
    /// should only be called when the item is in view
    #[inline]
    pub fn load_artwork(&self) {
        let imp = self.imp();
        let is_visible = Arc::clone(&imp.is_visible);
        is_visible.store(true, Ordering::Release);
        let index = self.index() as usize;
        let song = Arc::clone(imp.shared_song());

        Library::run_task(library_tx(), move || {
            if !is_visible.load(Ordering::Acquire) {
                return;
            }
            let mut song_info = song.info();
            drop(song_info.load_thumbnail(UsedBy::Library));
            let _ = ui_tx().send_blocking(UpdateUI::LibrarySongLoaded { index, song });
        });
    }

    /// Unloads the artwork thumbnail in a background thread
    ///
    /// The function also marks the item as not visible, so it
    /// should only be called when the item is not in view
    #[inline]
    pub fn unload_artwork(&self) {
        let imp = self.imp();
        let is_visible = Arc::clone(&imp.is_visible);
        is_visible.store(false, Ordering::Release);
        let song = Arc::clone(imp.shared_song());
        self.set_property("artwork", Option::<gdk::Texture>::None);

        // NOTE: Unloading in the background in case the `RwLock` is busy
        Library::run_task(library_tx(), move || {
            if is_visible.load(Ordering::Acquire) {
                return;
            }
            song.info().mark_thumbnail_unused_by(UsedBy::Library);
        });
    }

    /// Returns the `AtomicBool` for determining whether this item is in view
    #[inline]
    #[must_use]
    pub fn is_visible(&self) -> &Arc<AtomicBool> {
        &self.imp().is_visible
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
    pub fn order_cmp(&self, other: &Self, order_by: &LibrarySort) -> gtk::Ordering {
        (other.rank().total_cmp(&self.rank()))
            .then_with(|| order_by.cmp(self, other))
            .into()
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

impl LibraryObject for SongObject {
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

impl Sortable for SongObject {
    #[inline]
    fn sort_default(&self, other: &Self) -> core::cmp::Ordering {
        (self.artist().cmp(&other.artist())).then_with(|| self.index().cmp(&other.index()))
    }
    #[inline]
    fn sort_random(&self, other: &Self) -> core::cmp::Ordering {
        self.random().cmp(&other.random())
    }
}
