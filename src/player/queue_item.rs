use core::sync::atomic::{AtomicBool, Ordering};
use std::hint::unreachable_unchecked;
use std::sync::Arc;

use crate::library::{SharedSong, Song};

#[derive(Clone)]
pub enum QueueItem {
    Song(SharedSong),
    Stopper(SharedStopper),
}

#[derive(Clone)]
pub struct SharedStopper(Arc<AtomicBool>);
impl SharedStopper {
    #[inline]
    #[must_use]
    pub fn new(should_close: bool) -> SharedStopper {
        SharedStopper(Arc::new(AtomicBool::new(should_close)))
    }
    #[inline]
    #[must_use]
    pub fn should_close_player(&self) -> bool {
        self.0.load(Ordering::Relaxed)
    }
    #[inline]
    pub fn set_close_player(&self, should_close: bool) {
        self.0.store(should_close, Ordering::Relaxed);
    }
    #[inline]
    #[must_use]
    pub fn display_name(&self) -> &'static str {
        Self::display_name_from_bool(self.should_close_player())
    }
    #[inline]
    #[must_use]
    pub const fn display_name_from_bool(should_close: bool) -> &'static str {
        match should_close {
            // TODO: Support translations
            false => "Pause",
            true => "Pause & Close Player",
        }
    }
}

impl QueueItem {
    /// Assumes the `QueueItem` is a `Song`, and returns a
    /// reference to its inner `SharedSong` value
    ///
    /// # Panics
    /// The function panics if the `QueueItem` is not a `Song`
    ///
    /// Assuming proper behavior, `SongQueue::current().as_song()`
    /// should always succeed, since each `Stopper` is removed as
    /// soon as it is encountered
    #[inline]
    #[must_use]
    pub const fn as_song(&self) -> &SharedSong {
        match self {
            Self::Song(song) => song,
            Self::Stopper(_) => panic!("called `QueueItem::as_song()` on a `Stopper` value"),
        }
    }
    /// Assumes the `QueueItem` is a `Song`, and returns a
    /// reference to its inner `SharedSong` value, without
    /// checking if it is the correct variant
    ///
    /// # Safety
    /// The caller must be certain that the `QueueItem` is always
    /// a `Song`, otherwise this will result in undefined behavior
    #[inline]
    #[must_use]
    pub const unsafe fn as_song_unchecked(&self) -> &SharedSong {
        match self {
            Self::Song(song) => song,
            // SAFETY: Caller must ensure the value is a `Song`
            Self::Stopper(_) => unsafe { unreachable_unchecked() },
        }
    }
    /// Returns `true` if the `QueueItem` is a `Song`, or `false` if it is not
    #[inline]
    #[must_use]
    pub const fn is_song(&self) -> bool {
        matches!(self, Self::Song(_))
    }
    /// Returns the value from the `predicate` if `QueueItem` is a `Song`, or `false` if it is not
    #[inline]
    pub fn is_song_and<P>(&self, predicate: P) -> bool
    where
        P: FnOnce(&Song) -> bool,
    {
        match self {
            Self::Song(song) => predicate(song),
            Self::Stopper(_) => false,
        }
    }
    /// Assumes the `QueueItem` is a `Stopper`, and returns a
    /// reference to its inner `SharedStopper` value
    ///
    /// # Panics
    /// The function panics if the `QueueItem` is not a `Stopper`
    #[inline]
    #[must_use]
    pub const fn as_stopper(&self) -> &SharedStopper {
        match self {
            Self::Stopper(stopper) => stopper,
            Self::Song(_) => panic!("called `QueueItem::as_stopper()` on a `Song` value"),
        }
    }
    /// Returns `true` if the `QueueItem` is a `Stopper`
    #[inline]
    #[must_use]
    pub const fn is_stopper(&self) -> bool {
        matches!(self, Self::Stopper(_))
    }

    /// Runs a closure on the `QueueItem` if it is a `Song`,
    /// and returns the output of the closure inside an `Option`.
    /// If the `QueueItem` is not a `Song`, `None` is returned.
    #[inline]
    pub fn map_song<F, T>(&self, f: F) -> Option<T>
    where
        F: FnOnce(&Song) -> T,
    {
        match self {
            QueueItem::Song(song) => Some(f(song)),
            QueueItem::Stopper(_) => None,
        }
    }

    /// Creates a `QueueItem::Song` using the given `song`
    #[inline]
    #[must_use]
    pub fn from_song(song: &SharedSong) -> QueueItem {
        QueueItem::Song(Arc::clone(song))
    }

    /// Creates a `QueueItem::Stopper` using the given `stopper`
    #[inline]
    #[must_use]
    pub fn from_stopper(stopper: &SharedStopper) -> QueueItem {
        QueueItem::Stopper(stopper.clone())
    }

    /// Creates a new `QueueItem::Stopper` with the given `should_close` value
    #[inline]
    #[must_use]
    pub fn new_stopper(should_close: bool) -> QueueItem {
        QueueItem::Stopper(SharedStopper::new(should_close))
    }
}

impl PartialEq for QueueItem {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (QueueItem::Song(this), QueueItem::Song(other)) => Arc::ptr_eq(this, other),
            (QueueItem::Stopper(this), QueueItem::Stopper(other)) => Arc::ptr_eq(&this.0, &other.0),
            _ => false,
        }
    }
}

impl Default for QueueItem {
    /// Returns a new `Stopper` which does not close the player
    #[inline]
    fn default() -> Self {
        QueueItem::new_stopper(false)
    }
}
