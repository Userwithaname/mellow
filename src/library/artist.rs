use std::sync::{Arc, Mutex};

use crate::library::album::NewSharedAlbum;
use crate::library::{Album, SharedAlbum, SharedSong, SongInfo, ToQueue, ToShuffledQueue};
use crate::player::QueueItem;

pub struct Artist {
    pub(super) name: String,
    /// # Safety
    /// Costruct using `NewSharedArtist::new_artist_album_pair` to
    /// ensure `albums` is never empty to prevent undefined behavior
    albums: ArtistAlbums, // Private to enforce safety requirement
}

impl Artist {
    /// Raturns the artist's name
    #[inline]
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }
    /// Returns a reference the artist's albums
    #[inline]
    #[must_use]
    pub const fn albums(&self) -> &ArtistAlbums {
        &self.albums
    }
    /// Adds an album to the list of artist's albums
    #[inline]
    pub fn add_album(&mut self, album: SharedAlbum, sort_info: &SongInfo) {
        match self.albums.find_artist_album(sort_info) {
            Err(index) | Ok(index) => self.albums.insert(index, album),
        }
    }
    /// Returns the artist's newest album
    #[inline]
    #[must_use]
    pub fn newest_album(&self) -> &SharedAlbum {
        // SAFETY: An artist with no albums cannot be constructed
        unsafe { self.albums.last().unwrap_unchecked() }
    }

    /// Loops through all artist's albums and returns the average rating,
    /// or returns `fallback` if no songs have a rating assigned. Albums
    /// with no rating assigned do not contribute to the average.
    ///
    /// # Panics
    /// Panics if the album `Mutex` is poisoned
    #[inline]
    #[must_use]
    pub fn average_rating(&self, fallback: f64) -> f64 {
        let mut rating_total = 0.0;
        let mut num_albums = 0;
        for album in &self.albums {
            // NOTE: It would be more efficient to cache the average ratings on `UserAlbumInfo`
            let average_album_rating = album.lock().unwrap().average_rating(-1.0);
            if average_album_rating > 0.0 {
                rating_total += average_album_rating;
                num_albums += 1;
            }
        }
        match num_albums {
            0 => fallback,
            n => rating_total / n as f64,
        }
    }

    /// Loops through all artist's albums and returns the average rating,
    /// defaulting to `fallback` for songs which do not have a rating
    /// assigned
    ///
    /// # Panics
    /// Panics if the album `Mutex` is poisoned
    #[inline]
    #[must_use]
    pub fn sort_rating(&self, fallback: f64) -> f64 {
        let mut rating_total = 0.0;
        for album in &self.albums {
            // NOTE: It would be more efficient to cache the sort ratings on `UserAlbumInfo`
            rating_total += album.lock().unwrap().sort_rating(fallback);
        }
        rating_total / self.albums.len() as f64
    }
}

impl ToQueue for Artist {
    fn to_queue(&self) -> Vec<QueueItem> {
        self.albums.to_queue()
    }
}
impl ToShuffledQueue for Artist {
    fn to_shuffled_queue(&self) -> Vec<QueueItem> {
        self.albums.to_shuffled_queue()
    }
}

pub type SharedArtist = Arc<Mutex<Artist>>;
pub trait NewSharedArtist {
    fn new_artist_album_pair(info: &SongInfo, song: SharedSong) -> (SharedArtist, SharedAlbum);
}
impl NewSharedArtist for SharedArtist {
    /// Creates and returns a connected pair of `SharedArtist` and `SharedAlbum`,
    #[inline]
    fn new_artist_album_pair(info: &SongInfo, song: SharedSong) -> (SharedArtist, SharedAlbum) {
        let artist = Arc::new(Mutex::new(Artist {
            name: info.album_artist.clone(),
            albums: vec![], // SAFETY: The album is constructed and assigned before returning
        }));
        let album = SharedAlbum::new_album(info, song, Arc::clone(&artist));
        artist.lock().unwrap().albums.push(Arc::clone(&album));

        (artist, album)
    }
}

impl ToQueue for SharedArtist {
    fn to_queue(&self) -> Vec<QueueItem> {
        self.lock().unwrap().to_queue()
    }
}
impl ToShuffledQueue for SharedArtist {
    fn to_shuffled_queue(&self) -> Vec<QueueItem> {
        self.lock().unwrap().to_shuffled_queue()
    }
}

pub type ArtistAlbums = Vec<Arc<Mutex<Album>>>;
pub trait SortedArtistAlbums {
    /// Returns `Ok(index)` if the item was found
    ///
    /// # Errors
    /// If the item was not found, the returned `Err(index)`
    /// can be used to insert the item to the proper position
    fn find_artist_album(&self, info: &SongInfo) -> Result<usize, usize>;
}
impl SortedArtistAlbums for ArtistAlbums {
    fn find_artist_album(&self, info: &SongInfo) -> Result<usize, usize> {
        self.binary_search_by(|album| {
            let album = album.lock().unwrap();
            (album.year().cmp(&info.year)).then_with(|| album.title().cmp(&info.album))
        })
    }
}
