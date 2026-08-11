use core::cmp::Ordering;

use crate::ui::LibraryObject;

#[derive(Clone, Copy, Default)]
pub struct LibrarySort {
    pub ordering: LibrarySortMode,
    pub reversed: bool,
}
impl LibrarySort {
    /// Returns the relevant ordering of `a` compared to `b`
    #[inline]
    #[must_use]
    pub fn cmp<S: Sortable>(&self, a: &S, b: &S) -> Ordering {
        let cmp = match self.ordering {
            LibrarySortMode::Default => a.sort_default(b),
            LibrarySortMode::ReleaseDate => a.sort_release_date(b),
            LibrarySortMode::Modified => a.sort_modified_newer(b),
            LibrarySortMode::Added => a.sort_added_newer(b),
            LibrarySortMode::PlayCount => a.sort_most_played(b),
            LibrarySortMode::Rating => a.sort_best_rating(b),
            LibrarySortMode::Random => a.sort_random(b),
        };
        match self.reversed {
            false => cmp,
            true => cmp.reverse(),
        }
    }
}

#[derive(Clone, Copy, Default)]
pub enum LibrarySortMode {
    #[default]
    Default,
    Rating,
    PlayCount,
    ReleaseDate,
    Added,
    Modified,
    Random,
}

impl LibrarySortMode {
    #[inline]
    #[must_use]
    pub const fn to_str(self) -> &'static str {
        match self {
            LibrarySortMode::Default => "Default",
            LibrarySortMode::Rating => "Rating",
            LibrarySortMode::PlayCount => "Play Count",
            LibrarySortMode::ReleaseDate => "Release Date",
            LibrarySortMode::Added => "Added",
            LibrarySortMode::Modified => "Modified",
            LibrarySortMode::Random => "Random",
        }
    }
}
impl From<&str> for LibrarySortMode {
    #[inline]
    fn from(value: &str) -> Self {
        match value {
            "Default" => LibrarySortMode::Default,
            "Rating" => LibrarySortMode::Rating,
            "Play Count" => LibrarySortMode::PlayCount,
            "Release Date" => LibrarySortMode::ReleaseDate,
            "Added" => LibrarySortMode::Added,
            "Modified" => LibrarySortMode::Modified,
            "Random" => LibrarySortMode::Random,
            _ => LibrarySortMode::Default,
        }
    }
}

pub trait Sortable: LibraryObject {
    /// Default sorting order, also used as a fallback
    fn sort_default(&self, other: &Self) -> Ordering;
    /// Random sorting order
    ///
    /// Note: This should return a pre-determined value,
    /// rather than a new one each time
    fn sort_random(&self, other: &Self) -> Ordering;

    #[inline]
    #[must_use]
    fn sort_release_date(&self, other: &Self) -> Ordering {
        (other.year().cmp(&self.year())).then_with(|| self.sort_default(other))
    }
    #[inline]
    #[must_use]
    fn sort_modified_newer(&self, other: &Self) -> Ordering {
        (other.modified().cmp(&self.modified())).then_with(|| self.sort_default(other))
    }
    #[inline]
    #[must_use]
    fn sort_added_newer(&self, other: &Self) -> Ordering {
        (other.added().cmp(&self.added())).then_with(|| self.sort_default(other))
    }
    #[inline]
    #[must_use]
    fn sort_most_played(&self, other: &Self) -> Ordering {
        (other.play_count().total_cmp(&self.play_count())).then_with(|| self.sort_default(other))
    }
    #[inline]
    #[must_use]
    fn sort_best_rating(&self, other: &Self) -> Ordering {
        (other.rating().total_cmp(&self.rating())).then_with(|| self.sort_most_played(other))
    }
}
