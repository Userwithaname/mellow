use std::fmt::Display;
use std::num::ParseIntError;
use std::str::FromStr;

/// A type for storing star ratings and marking as favorite
#[derive(Default, Debug, Clone, Copy)]
pub struct SongRating(u8);

impl SongRating {
    const STARS_MASK: u8 = 0b00000111;
    const FAVORITE_MASK: u8 = 0b00001000;

    /// Returns a new `SongRating`
    ///
    /// # Example:
    /// ```rust
    /// use mellow::library::song_rating::SongRating;
    ///
    /// fn main() {
    ///     let mut rating = SongRating::new(3, true);
    ///     assert_eq!(rating.stars(), 3);
    ///     assert_eq!(rating.is_favorite(), true);
    /// }
    /// ```
    #[inline]
    #[must_use]
    pub fn new(stars: u8, favorite: bool) -> SongRating {
        debug_assert!(
            stars <= 5,
            "Rating cannot be {stars} stars; must be 5 stars or less"
        );
        SongRating(stars | (favorite as u8 * Self::FAVORITE_MASK))
    }

    /// Returns the number of stars assigned to the rating
    #[inline]
    #[must_use]
    pub const fn stars(&self) -> u8 {
        self.0 & Self::STARS_MASK
    }

    /// Returns `true` if marked as favorite, or `false` if not
    #[inline]
    #[must_use]
    pub const fn is_favorite(&self) -> bool {
        self.0 & Self::FAVORITE_MASK == Self::FAVORITE_MASK
    }

    /// Returns raw `u8` rating representation
    #[inline]
    #[must_use]
    pub const fn as_raw(&self) -> u8 {
        self.0
    }

    /// Sets the stars to the given value, keeping the previous favorite value
    ///
    /// # Example:
    /// ```rust
    /// use mellow::library::song_rating::SongRating;
    ///
    /// fn main() {
    ///     let mut rating = SongRating::new(3, true);
    ///
    ///     rating.set_stars(5);
    ///     assert_eq!(rating.stars(), 5);
    ///     assert_eq!(rating.is_favorite(), true);
    /// }
    /// ```
    #[inline]
    pub fn set_stars(&mut self, stars: u8) {
        debug_assert!(
            stars <= 5,
            "Rating cannot be {stars} stars; must be 5 stars or less"
        );
        self.0 = stars + (self.0 & Self::FAVORITE_MASK);
    }

    /// Marks the rating as favorite or non-favorite without changing the stars
    ///
    /// # Example:
    /// ```rust
    /// use mellow::library::song_rating::SongRating;
    ///
    /// fn main() {
    ///     let mut rating = SongRating::new(3, true);
    ///
    ///     rating.set_favorite(false);
    ///     assert_eq!(rating.is_favorite(), false);
    ///     assert_eq!(rating.stars(), 3);
    ///
    ///     rating.set_favorite(true);
    ///     assert_eq!(rating.is_favorite(), true);
    /// }
    /// ```
    #[inline]
    pub const fn set_favorite(&mut self, favorite: bool) {
        self.0 = self.0 & Self::STARS_MASK | (favorite as u8 * Self::FAVORITE_MASK);
    }

    /// Merges the rating from `other` into `self`
    ///
    /// - Stars are set to the average value, or whichever one is non-zero is used
    /// - Marks as favorite if either `self` or `other` is marked as such
    #[inline]
    pub const fn merge_with(&mut self, other: &SongRating) {
        let (own_stars, other_stars) = (self.stars(), other.stars());
        let favorite = self.0 & other.0 & Self::FAVORITE_MASK;
        self.0 = if own_stars == 0 {
            other_stars | favorite
        } else if other_stars == 0 {
            own_stars | favorite
        } else {
            own_stars.midpoint(other_stars) | favorite
        };
    }
}

impl FromStr for SongRating {
    type Err = ParseIntError;

    #[inline]
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        s.parse().map(SongRating)
    }
}

impl Display for SongRating {
    #[inline]
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}
