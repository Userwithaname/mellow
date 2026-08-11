use adw::subclass::prelude::*;
use core::cmp;
use glib::Object;
use gtk::{gdk, glib};

use crate::library::SharedArtist;
use crate::ui::{LibraryObject, LibrarySort, Sortable};

mod imp;

glib::wrapper! {
    /// # Safety
    /// Either construct using `ArtistObject::new()`, or ensure
    /// that `….imp().shared_artist` is initialized if constructing
    /// manually. Failing to do so will lead to undefined behavior.
    pub struct ArtistObject(ObjectSubclass<imp::ArtistObject>);
}

impl ArtistObject {
    #[inline]
    #[must_use]
    pub fn new(index: u32, artist: &str, albums: u64, shared_artist: SharedArtist) -> Self {
        let artist_object: ArtistObject = Object::builder()
            .property("index", index)
            .property("artist", artist)
            .property("albums", albums)
            .build();
        let _ = artist_object.imp().shared_artist.set(shared_artist);
        artist_object
    }

    /// Returns the `SharedArtist` associated with this object
    #[inline]
    #[must_use]
    pub fn shared_artist(&self) -> &SharedArtist {
        self.imp().shared_artist()
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
pub struct ArtistData {
    index: u32,
    artist: String,
    albums: u64,
    artwork: Option<gdk::Paintable>,
    rank: f64,
    /// Stars rating (0 if unassigned)
    stars: f64,
    /// Rating with a fallback value (3 if unassigned, used for sorting)
    rating: f64,
    played: f64,
    modified: u64,
    added: u64,
    random: u64,
    tags: Vec<String>,
}

impl LibraryObject for ArtistObject {
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
        0 // Not applicable
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

impl Sortable for ArtistObject {
    #[inline]
    fn sort_default(&self, other: &Self) -> cmp::Ordering {
        self.index().cmp(&other.index())
    }
    #[inline]
    fn sort_random(&self, other: &Self) -> cmp::Ordering {
        self.random().cmp(&other.random())
    }
}
