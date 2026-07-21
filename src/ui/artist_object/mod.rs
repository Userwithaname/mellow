use adw::{prelude::*, subclass::prelude::*};
use core::cmp;
use glib::Object;
use gtk::{gdk, glib};

use crate::library::SharedArtist;
use crate::ui::{FilterMode, SortConfig};
use crate::util::CmpIsEqOr;

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

    #[inline]
    pub const fn load_artwork(&self) {
        // TODO: Decide what kind of image to show for library artists and construct it
        // Maybe 4 artworks composed in a grid with a circular cutout might look good
        // if self.artwork().is_some() {
        //     return;
        // }
        // let index = self.index() as usize;
        // Library::run_task(LIBRARY_TX.get().expect(EXP_INIT), move || {
        //     UI_TX
        //         .get()
        //         .expect(EXP_INIT)
        //         .send(UpdateUI::LibraryArtistLoaded(index))
        //         .expect(EXP_RX);
        // });
    }

    #[inline]
    pub fn unload_artwork(&self) {
        self.set_property("artwork", Option::<gdk::Texture>::None);
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
    pub fn order_cmp(&self, other: &Self, order_by: SortConfig<ArtistOrdering>) -> gtk::Ordering {
        let ord = match other.rank().total_cmp(&self.rank()) {
            cmp::Ordering::Equal => match order_by.ordering.get() {
                ArtistOrdering::Default => self.cmp_artist(other),
                ArtistOrdering::PlayCount => self.cmp_most_played(other),
                ArtistOrdering::Rating => self.cmp_best_rating(other),
                ArtistOrdering::Added => self.cmp_added_newer(other),
                ArtistOrdering::Modified => self.cmp_modified_newer(other),
                ArtistOrdering::Random => self.cmp_random(other),
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
    fn cmp_artist(&self, other: &Self) -> cmp::Ordering {
        self.artist().cmp(&other.artist())
    }
    #[inline]
    #[must_use]
    fn cmp_most_played(&self, other: &Self) -> cmp::Ordering {
        (other.played().total_cmp(&self.played())).then_with(|| self.index().cmp(&other.index()))
    }
    #[inline]
    #[must_use]
    fn cmp_best_rating(&self, other: &Self) -> cmp::Ordering {
        (other.rating().total_cmp(&self.rating())).then_with(|| self.cmp_most_played(other))
    }
    #[inline]
    #[must_use]
    fn cmp_added_newer(&self, other: &Self) -> cmp::Ordering {
        // NOTE: Comparing added time using the oldest
        // album's first song is not necessarily correct
        (other.added().cmp(&self.added())).then_with(|| self.index().cmp(&other.index()))
    }
    #[inline]
    #[must_use]
    fn cmp_modified_newer(&self, other: &Self) -> cmp::Ordering {
        (other.modified().cmp(&self.modified())).then_with(|| self.index().cmp(&other.index()))
    }
    #[inline]
    #[must_use]
    fn cmp_random(&self, other: &Self) -> cmp::Ordering {
        other.random().cmp(&self.random())
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
}

#[derive(Clone, Copy)]
pub enum ArtistOrdering {
    Default,
    Rating,
    PlayCount,
    Added,
    Modified,
    Random,
}

impl ArtistOrdering {
    #[inline]
    #[must_use]
    pub const fn to_str(self) -> &'static str {
        match self {
            ArtistOrdering::Default => "Default",
            ArtistOrdering::Rating => "Rating",
            ArtistOrdering::PlayCount => "Play Count",
            ArtistOrdering::Added => "Added",
            ArtistOrdering::Modified => "Modified",
            ArtistOrdering::Random => "Random",
        }
    }
}
impl From<&str> for ArtistOrdering {
    #[inline]
    fn from(value: &str) -> Self {
        match value {
            "Default" => ArtistOrdering::Default,
            "Rating" => ArtistOrdering::Rating,
            "Play Count" => ArtistOrdering::PlayCount,
            "Added" => ArtistOrdering::Added,
            "Modified" => ArtistOrdering::Modified,
            "Random" => ArtistOrdering::Random,
            _ => unimplemented!(),
        }
    }
}

#[derive(Default)]
pub struct ArtistFilters {
    pub filter_mode: FilterMode,
    pub rating: Option<(cmp::Ordering, u8)>,
    pub play_count: Option<(cmp::Ordering, u64)>,
}

impl ArtistFilters {
    #[inline]
    pub fn filter(&self, song_object: &ArtistObject) -> bool {
        match self.filter_mode {
            FilterMode::Exclusive => self.filter_exclusive(song_object),
            FilterMode::Inclusive => self.filter_inclusive(song_object),
        }
    }
    pub fn filter_exclusive(&self, artist_object: &ArtistObject) -> bool {
        self.rating.is_none_or(|rating| {
            (artist_object.stars().total_cmp(&(rating.1 as f64))).is_eq_or(rating.0)
        }) && self.play_count.is_none_or(|play_count| {
            (artist_object.played().total_cmp(&(play_count.1 as f64))).is_eq_or(play_count.0)
        })
        // && (self.tags.is_empty()
        //     || (self.tags.iter()).any(|tag| artist_object.tags().contains(tag)))
    }
    pub fn filter_inclusive(&self, artist_object: &ArtistObject) -> bool {
        (self.rating.is_none() && self.play_count.is_none())
            || self.rating.is_some_and(|rating| {
                (artist_object.stars().total_cmp(&(rating.1 as f64))).is_eq_or(rating.0)
            })
            || self.play_count.is_some_and(|play_count| {
                (artist_object.played().total_cmp(&(play_count.1 as f64))).is_eq_or(play_count.0)
            })
        // && (self.tags.is_empty()
        //     || (self.tags.iter()).any(|tag| artist_object.tags().contains(tag)))
    }
}
