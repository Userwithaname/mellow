use core::cell::Cell;
use gtk::gdk;
use std::sync::OnceLock;
use tokio::sync::mpsc as tokio_mpsc;

mod actions;
mod album_object;
mod album_page;
mod albums_page;
mod application;
mod artist_object;
mod artist_page;
mod artists_page;
mod item_row;
mod item_tile;
mod library_page;
mod list_row;
mod lyrics_page;
mod main_player;
mod queue_item_object;
mod queue_page;
mod queue_subpage;
mod rating;
mod settings_page;
mod song_object;
mod song_page;
mod songs_page;
mod window;

pub use album_object::{AlbumData, AlbumObject, AlbumOrdering};
pub use album_page::AlbumPage;
pub use albums_page::AlbumsPage;
pub use application::Application;
pub use artist_object::{ArtistData, ArtistObject, ArtistOrdering};
pub use artist_page::ArtistPage;
pub use artists_page::ArtistsPage;
pub use item_row::ItemRow;
pub use item_tile::ItemTile;
pub use library_page::{LibraryPage, SubpageType};
pub use list_row::ListRow;
pub use lyrics_page::LyricsPage;
pub use main_player::MainPlayer;
pub use queue_item_object::{QueueItemData, QueueItemObject};
pub use queue_page::QueuePage;
pub use queue_subpage::QueueSubpage;
pub use rating::Rating;
pub use settings_page::{SettingsPage, StartupQueueChoice};
pub use song_object::{SongData, SongObject, SongOrdering};
pub use song_page::SongPage;
pub use songs_page::SongsPage;
pub use window::Window;

use crate::excuses::EXP_RX;
use crate::library::{Albums, Artists, Songs, ToQueue};
use crate::library::{SharedAlbum, SharedArtist, SharedSong};
use crate::player::QueueItem;

static UI_TX: OnceLock<tokio_mpsc::UnboundedSender<UpdateUI>> = OnceLock::new();
/// Returns the channel sender for sending requests to the UI using `UpdateUI`
///
/// # Safety
/// Causes undefined behavior if called before `init_channels`
#[inline]
pub fn ui_tx() -> &'static tokio_mpsc::UnboundedSender<UpdateUI> {
    // SAFETY: `init_channels` runs in `Application::run`, before starting any threads
    unsafe { UI_TX.get().unwrap_unchecked() }
}
/// Initializes the UI channel sender accessed through `ui_tx()`
///
/// # Errors
/// The function returns an error if `UI_TX` has already been initialized
#[inline]
pub fn init_ui_tx(
    ui_tx: tokio_mpsc::UnboundedSender<UpdateUI>,
) -> Result<(), tokio_mpsc::UnboundedSender<UpdateUI>> {
    UI_TX.set(ui_tx)
}

pub type ToastButtonAction = Box<(&'static str, Box<dyn Fn() + Send + 'static>)>;
pub enum UpdateUI {
    PlayerState {
        playing: bool,
        interactive: bool,
    },
    /// Current song time in milliseconds
    PlayerTime {
        time: Option<u64>,
    },
    /// Updates the song info displayed in the UI to the given item (use `Stopper` to reset)
    SongInfo {
        item: QueueItem,
        pause_after: bool,
    },
    /// Replaces the UI song queue with a new one, with the playing index as the second argument
    SetQueue {
        queue: Box<[QueueItem]>,
        playing_index: usize,
    },
    /// Updates the playing song index and redraws the queue
    SetQueueIndex(usize),
    /// Recenters the queue interface scroll position to keep the same items in view
    /// Use before changing the playing song index to the same value as used here
    RecenterQueue(isize),
    /// Redraws the current queue
    RedrawQueue,
    /// Exits queue selection mode if currently active
    ExitQueueSelection,
    /// Opens the subpage for the queue song at the given index
    OpenQueueSubpage(usize),
    /// Attempts to correct the queue subpage item index if the subpage item and index are
    /// inconsistent with the current queue, or closes the subpage if the item cannot be found
    ///
    /// Note: The UI queue must be updated first
    ValidateQueueSubpageIndex,
    /// Closes the subpage if it is open
    CloseQueueSubpage,
    /// Informs the UI of the new shuffle mode (so icons can be updated)
    Shuffle(bool),
    /// Informs the UI of the new repeat mode (so icons can be updated)
    Repeat(bool),

    /// Updates the directory list on the settings page
    SetLibraryDirs(Vec<String>),
    /// Updates the library songs
    SetLibrarySongs(Songs),
    /// Updates the library albums
    SetLibraryAlbums(Albums),
    /// Updates the library artists
    SetLibraryArtists(Artists),

    /// Prompts the library UI to assign the now-loaded song artwork for the item at index
    LibrarySongLoaded {
        index: usize,
        song: SharedSong,
    },
    /// Prompts the library UI to assign the now-loaded album artwork for the item at index
    /// The `song` field is the song the info will be read from (usually first song)
    LibraryAlbumLoaded {
        index: usize,
        song: SharedSong,
    },
    /// Prompts the library UI to assign the now-loaded artist artwork for the item at index
    LibraryArtistLoaded {
        index: usize,
    },
    /// Prompts the queue UI to assign the now-loaded song artwork for the item at index
    QueueSongLoaded {
        index: usize,
        song: SharedSong,
    },
    /// Prompts the album page UI to assign the now-loaded album artwork for the page at index
    /// The `song` field is the song the info will be read from (usually first song)
    AlbumPageLoaded {
        index: usize,
        song: SharedSong,
    },

    /// Opens the library song page for the item at the given index
    SongPageByIndex(usize),
    // Maybe `dyn Fn() -> Vec<QueueItem>` would be more useful?
    // Or `Vec<QueueItem>` directly, which would also remove the
    // need for the second field
    /// Opens a song page, with the following arguments:
    /// (index: `usize`, song: `SharedSong`, a closure returning the queue for starting playback)
    SongPage(Box<(usize, SharedSong, Box<dyn ToQueue + Send>)>),
    /// Opens an album page using a `SharedAlbum`
    AlbumPage(SharedAlbum),
    /// Opens an album page using a `SharedArtist`
    ArtistPage(SharedArtist),

    /// Focuses the 'Library' tab
    FocusLibrary,
    /// Focuses the 'Playing' tab
    FocusPlaying,
    /// Focuses the 'Settings' tab
    FocusSettings,
    /// Opens or closes the bottom sheet overlay
    OpenSheet(bool),
    /// Sets whether the sheet can be closed or not
    CanCloseSheet(bool),

    /// Runs a `gio` action
    RunAction(&'static str),
    /// Shows a progress bar with the specified progress value, or hides it
    Progress(Option<f64>),
    /// Displays the notification message (optionally takes a button name and action closure)
    Notification(String, Option<ToastButtonAction>),
    /// Dismisses all visible toast notifications
    DismissNotifications,

    /// Causes the channel to ignore any further requests (but does not close it)
    Uninit,

    /// Displays an error message informing the user that a component has crashed
    CrashNotice(String),
}

/// Shows the 'Playing' tab in the UI
///
/// # Panics
/// The function panics if the UI channel is closed
pub fn show_queue() {
    let ui_tx = ui_tx();
    ui_tx.send(UpdateUI::FocusPlaying).expect(EXP_RX);

    // NOTE: This will not close the lyrics page, if open
    let _ = ui_tx.send(UpdateUI::CloseQueueSubpage);

    // IDEA: Also scroll to a specified item in the queue

    // Re-open the overlay in case it was closed
    let _ = ui_tx.send(UpdateUI::OpenSheet(true));
}

// IDEA: The fallback images could be cached somehow
// (might be tricky since `gdk::Paintable` cannot be const)

// Returns a fallback image intended for artists with missing artwork
#[must_use]
pub fn fallback_artist_image() -> gdk::Paintable {
    // TODO: Fallback image for albums (maybe a symbolic disc icon?)
    gdk::Paintable::new_empty(1, 1)
}

// Returns a fallback image intended for albums with missing artwork
#[must_use]
pub fn fallback_album_image() -> gdk::Paintable {
    // TODO: Fallback image for albums (maybe a symbolic disc icon?)
    gdk::Paintable::new_empty(1, 1)
}

// Returns a fallback image intended for songs with missing album covers
#[must_use]
pub fn fallback_song_image() -> gdk::Paintable {
    // TODO: Fallback image for songs (maybe a symbolic note icon?)
    gdk::Paintable::new_empty(1, 1)
}

#[derive(Clone, Copy)]
pub struct SortConfig<O: 'static> {
    pub ordering: &'static Cell<O>,
    pub reversed: &'static Cell<bool>,
}
impl<O> SortConfig<O> {
    /// Constructs a new instance of `SortConfig`
    ///
    /// Note: Once constructed, the data will remain
    /// in memory for the duration of the program
    #[inline]
    pub fn new(ordering: O, reversed: bool) -> SortConfig<O> {
        SortConfig {
            ordering: Box::leak(Box::new(Cell::new(ordering))),
            reversed: Box::leak(Box::new(Cell::new(reversed))),
        }
    }
}

#[derive(Default)]
pub enum FilterMode {
    #[default]
    Exclusive,
    Inclusive,
}
