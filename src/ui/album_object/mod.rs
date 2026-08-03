use adw::subclass::prelude::*;
use core::{cmp, sync::atomic};
use glib::{Object, object::ObjectExt};
use gtk::{gdk, glib};
use std::sync::Arc;

use crate::library::tag_list::Tags;
use crate::library::{Library, SharedAlbum, library_tx};
use crate::ui::{FilterMode, SortConfig, UpdateUI, ui_tx};
use crate::util::CmpIsEqOr;

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
    pub fn order_cmp(&self, other: &Self, order_by: SortConfig<AlbumOrdering>) -> gtk::Ordering {
        let ord = match other.rank().total_cmp(&self.rank()) {
            cmp::Ordering::Equal => match order_by.ordering.get() {
                AlbumOrdering::Default => self.cmp_artist_year_album(other),
                AlbumOrdering::ReleaseDate => self.cmp_release_date(other),
                AlbumOrdering::Modified => self.cmp_modified_newer(other),
                AlbumOrdering::Added => self.cmp_added_newer(other),
                AlbumOrdering::PlayCount => self.cmp_most_played(other),
                AlbumOrdering::Rating => self.cmp_best_rating(other),
                AlbumOrdering::Random => self.cmp_random(other),
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
        (self.artist().cmp(&other.artist()))
            .then_with(|| self.year().cmp(&other.year()))
            .then_with(|| self.album().cmp(&other.album()))
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
    fn cmp_release_date(&self, other: &Self) -> cmp::Ordering {
        (other.year().cmp(&self.year())).then_with(|| self.index().cmp(&other.index()))
    }
    #[inline]
    #[must_use]
    fn cmp_modified_newer(&self, other: &Self) -> cmp::Ordering {
        // NOTE: Comparing modification time using the first song is not necessarily correct
        (other.modified().cmp(&self.modified())).then_with(|| self.cmp_artist_year_album(other))
    }
    #[inline]
    #[must_use]
    fn cmp_added_newer(&self, other: &Self) -> cmp::Ordering {
        (other.added().cmp(&self.added())).then_with(|| self.cmp_artist_year_album(other))
    }
    #[inline]
    #[must_use]
    fn cmp_random(&self, other: &Self) -> cmp::Ordering {
        other.random().cmp(&self.random())
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

#[derive(Clone, Copy)]
pub enum AlbumOrdering {
    Default,
    ReleaseDate,
    Modified,
    Added,
    Rating,
    PlayCount,
    Random,
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
            AlbumOrdering::Random => "Random",
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
            "Random" => AlbumOrdering::Random,
            _ => unimplemented!(),
        }
    }
}

#[derive(Default)]
pub struct AlbumFilters {
    pub filter_mode: FilterMode,
    pub rating: Option<(cmp::Ordering, u8)>,
    pub play_count: Option<(cmp::Ordering, u64)>,
    pub year: Option<(cmp::Ordering, u32)>,
    pub tag_filter_mode: FilterMode,
    pub tags: Tags,
}

impl AlbumFilters {
    #[inline]
    pub fn filter(&self, song_object: &AlbumObject) -> bool {
        match self.filter_mode {
            FilterMode::Exclusive => self.filter_exclusive(song_object),
            FilterMode::Inclusive => self.filter_inclusive(song_object),
        }
    }
    pub fn filter_exclusive(&self, album_object: &AlbumObject) -> bool {
        self.rating.is_none_or(|rating| {
            (album_object.stars().total_cmp(&(rating.1 as f64))).is_eq_or(rating.0)
        }) && self.play_count.is_none_or(|play_count| {
            (album_object.played().total_cmp(&(play_count.1 as f64))).is_eq_or(play_count.0)
        }) && (self.year).is_none_or(|year| album_object.year().cmp(&year.1).is_eq_or(year.0))
            && (self.tags.is_empty() || self.filter_tags(album_object))
    }
    pub fn filter_inclusive(&self, album_object: &AlbumObject) -> bool {
        ((self.rating.is_none() && self.play_count.is_none() && self.year.is_none())
            || self.rating.is_some_and(|rating| {
                (album_object.stars().total_cmp(&(rating.1 as f64))).is_eq_or(rating.0)
            })
            || self.play_count.is_some_and(|play_count| {
                (album_object.played().total_cmp(&(play_count.1 as f64))).is_eq_or(play_count.0)
            })
            || (self.year).is_some_and(|year| album_object.year().cmp(&year.1).is_eq_or(year.0)))
            && (self.tags.is_empty() || self.filter_tags(album_object))
    }
    pub fn filter_tags(&self, album_object: &AlbumObject) -> bool {
        let mut album_tags = Tags::from(album_object.tags());
        match self.tag_filter_mode {
            FilterMode::Exclusive => {
                if album_tags.is_empty() && *self.tags == ["untagged"] {
                    return true;
                }
                for tag in &*self.tags {
                    if !album_tags.contains(tag) {
                        album_tags.remove(tag);
                        return false;
                    }
                }
                true
            }
            FilterMode::Inclusive => self.tags.iter().any(|tag| {
                album_tags.contains(tag) || album_tags.is_empty() && tag == "untagged" //
            }),
        }
    }
}
