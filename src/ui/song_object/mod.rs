use adw::{prelude::*, subclass::prelude::*};
use core::{cmp, sync::atomic};
use glib::Object;
use gtk::{gdk, glib};
use std::sync::Arc;

use crate::cold_expression;
use crate::library::tag_list::Tags;
use crate::library::{Library, SharedSong, library_tx};
use crate::ui::{FilterMode, SortConfig, UpdateUI, ui_tx};
use crate::util::CmpIsEqOr;

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
            let _ = ui_tx().send_blocking(UpdateUI::LibrarySongLoaded { index, song });
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
        (self.artist().cmp(&other.artist())).then_with(|| self.index().cmp(&other.index()))
    }
    #[inline]
    #[must_use]
    fn cmp_best_rating(&self, other: &Self) -> cmp::Ordering {
        (other.rating().cmp(&self.rating())).then_with(|| self.cmp_most_played(other))
    }
    #[inline]
    #[must_use]
    fn cmp_most_played(&self, other: &Self) -> cmp::Ordering {
        (other.played().cmp(&self.played())).then_with(|| self.cmp_default(other))
    }
    #[inline]
    #[must_use]
    fn cmp_release_date(&self, other: &Self) -> cmp::Ordering {
        (other.year().cmp(&self.year())).then_with(|| self.cmp_default(other))
    }
    #[inline]
    #[must_use]
    fn cmp_added_newer(&self, other: &Self) -> cmp::Ordering {
        (other.modified().cmp(&self.modified())).then_with(|| self.cmp_default(other))
    }
    #[inline]
    #[must_use]
    fn cmp_modified_newer(&self, other: &Self) -> cmp::Ordering {
        (other.modified().cmp(&self.modified())).then_with(|| self.cmp_default(other))
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
    /// Rating, as displayed in the UI (0 if unassigned)
    stars: u8,
    /// Rating with a fallback value (3 if unassigned, used for sorting)
    rating: u8,
    played: u64,
    modified: u64,
    added: u64,
    random: u64,
    tags: Vec<String>,
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
            "Random" => SongOrdering::Random,
            _ => unimplemented!(),
        }
    }
}

#[derive(Default)]
pub struct SongFilters {
    pub filter_mode: FilterMode,
    pub rating: Option<(cmp::Ordering, u8)>,
    pub play_count: Option<(cmp::Ordering, u64)>,
    pub year: Option<(cmp::Ordering, u32)>,
    pub tag_filter_mode: FilterMode,
    pub tags: Tags,
}

impl SongFilters {
    #[inline]
    pub fn filter(&self, song_object: &SongObject) -> bool {
        match self.filter_mode {
            FilterMode::Exclusive => self.filter_exclusive(song_object),
            FilterMode::Inclusive => self.filter_inclusive(song_object),
        }
    }
    pub fn filter_exclusive(&self, song_object: &SongObject) -> bool {
        self.rating
            .is_none_or(|rating| song_object.stars().cmp(&rating.1).is_eq_or(rating.0))
            && self.play_count.is_none_or(|play_count| {
                (song_object.played().cmp(&play_count.1)).is_eq_or(play_count.0)
            })
            && (self.year).is_none_or(|year| song_object.year().cmp(&year.1).is_eq_or(year.0))
            && (self.tags.is_empty() || self.filter_tags(song_object))
    }
    pub fn filter_inclusive(&self, song_object: &SongObject) -> bool {
        ((self.rating.is_none() && self.play_count.is_none() && self.year.is_none())
            || self
                .rating
                .is_some_and(|rating| song_object.stars().cmp(&rating.1) == rating.0)
            || self
                .play_count
                .is_some_and(|play_count| song_object.played().cmp(&play_count.1) == play_count.0)
            || self
                .year
                .is_some_and(|year| song_object.year().cmp(&year.1) == year.0))
            && (self.tags.is_empty() || self.filter_tags(song_object))
    }
    pub fn filter_tags(&self, song_object: &SongObject) -> bool {
        let mut song_tags = Tags::from(song_object.tags());
        match self.tag_filter_mode {
            FilterMode::Exclusive => {
                if song_tags.is_empty() && *self.tags == ["untagged"] {
                    return true;
                }
                for tag in &*self.tags {
                    if !song_tags.contains(tag) {
                        song_tags.remove(tag);
                        return false;
                    }
                }
                true
            }
            FilterMode::Inclusive => self.tags.iter().any(|tag| {
                song_tags.contains(tag) || song_tags.is_empty() && tag == "untagged" //
            }),
        }
    }
}
