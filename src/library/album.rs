use std::sync::{Arc, Mutex};

use crate::library::{SharedArtist, SharedSong, Song, SongInfo, ToQueue};
use crate::player::QueueItem;

// TODO: Save/load album info (such as ratings)

pub struct Album {
    pub(super) title: String,
    pub(super) year: u16,
    /// # Safety
    /// Costruct using `NewSharedAlbum::new_album` to ensure
    /// `songs` is never empty to prevent undefined behavior
    songs: AlbumSongs, // Private to enforce safety requirement
    pub(super) artist: SharedArtist,
}

impl Album {
    /// Returns the album's title
    #[inline]
    #[must_use]
    pub const fn title(&self) -> &String {
        &self.title
    }
    /// Returns the album's release year
    #[inline]
    #[must_use]
    pub const fn year(&self) -> u16 {
        self.year
    }
    /// Returns a reference to the album songs
    #[inline]
    #[must_use]
    pub const fn songs(&self) -> &AlbumSongs {
        &self.songs
    }
    /// Adds a song to the list of album songs
    #[inline]
    pub fn add_song(&mut self, song: SharedSong, sort_info: &SongInfo) {
        match self.songs.find_album_song(sort_info) {
            Err(index) | Ok(index) => self.songs.insert(index, song),
        }
    }
    /// Returns a reference to the album's artist
    #[inline]
    #[must_use]
    pub const fn artist(&self) -> &SharedArtist {
        &self.artist
    }
    /// Returns a cloned `Arc` for the album's artist
    #[inline]
    #[must_use]
    pub fn artist_cloned(&self) -> SharedArtist {
        Arc::clone(&self.artist)
    }
    /// Returns a reference to the first song on the album
    #[inline]
    #[must_use]
    pub fn first_song(&self) -> &SharedSong {
        // SAFETY: An album with no songs cannot be constructed
        unsafe { self.songs.get_unchecked(0) }
    }

    /// Loops through all album songs and returns the average rating,
    /// or returns `fallback` if no songs have a rating assigned. Songs
    /// with no rating assigned do not contribute to the average.
    #[must_use]
    pub fn average_rating(&self, fallback: f64) -> f64 {
        let mut rating_total = 0.0;
        let mut num_songs = 0;
        for song in &self.songs {
            match song.info().user().rating {
                0 => continue,
                n => rating_total += n as f64,
            }
            num_songs += 1;
        }
        match num_songs {
            0 => fallback,
            n => rating_total / n as f64,
        }
    }
    /// Loops through all album songs and returns the average rating,
    /// defaulting to `fallback` for songs which do not have a rating
    /// assigned
    #[must_use]
    pub fn sort_rating(&self, fallback: f64) -> f64 {
        let mut rating_total = 0.0;
        for song in &self.songs {
            match song.info().user().rating {
                0 => rating_total += fallback,
                n => rating_total += n as f64,
            }
        }
        rating_total / self.songs.len() as f64
    }
    /// Loops through all album songs and returns the average play count
    #[must_use]
    pub fn average_play_count(&self) -> f64 {
        let mut play_count_total = 0;
        for song in &self.songs {
            play_count_total += song.info().user().play_count;
        }
        match self.songs.len() {
            0 => 0.0,
            n => play_count_total as f64 / n as f64,
        }
    }

    /// Sets the rating of all songs on the album to `rating`
    pub fn rate_all_songs(&self, rating: u8) {
        for song in &self.songs {
            song.info().set_rating(rating);
        }
    }
}

impl ToQueue for Album {
    fn to_queue(&self) -> Vec<QueueItem> {
        self.songs.to_queue()
    }
}

pub type SharedAlbum = Arc<Mutex<Album>>;
pub trait NewSharedAlbum {
    fn new_album(info: &SongInfo, song: SharedSong, artist: SharedArtist) -> SharedAlbum;
}
impl NewSharedAlbum for SharedAlbum {
    /// Creates and returns a new `SharedAlbum` using the provided arguments
    #[inline]
    fn new_album(info: &SongInfo, song: SharedSong, artist: SharedArtist) -> SharedAlbum {
        Arc::new(Mutex::new(Album {
            title: info.album.clone(),
            year: info.year,
            songs: vec![song],
            artist,
        }))
    }
}

impl ToQueue for SharedAlbum {
    fn to_queue(&self) -> Vec<QueueItem> {
        self.lock().unwrap().to_queue()
    }
}

pub type AlbumSongs = Vec<Arc<Song>>;
pub trait SortedAlbumSongs {
    /// Returns `Ok(index)` if the item was found
    ///
    /// # Errors
    /// If the item was not found, the returned `Err(index)`
    /// can be used to insert the item to the proper position
    fn find_album_song(&self, info: &SongInfo) -> Result<usize, usize>;
}
impl SortedAlbumSongs for AlbumSongs {
    #[inline]
    fn find_album_song(&self, info: &SongInfo) -> Result<usize, usize> {
        self.binary_search_by(|song| {
            let mut new_info = song.info();
            new_info.load_basic_and(|new_info| {
                (new_info.disc.cmp(&info.disc)).then_with(|| new_info.track.cmp(&info.track))
            })
        })
    }
}
