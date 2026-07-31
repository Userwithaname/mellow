use core::error::Error;
use gdk::{gdk_pixbuf::Pixbuf, prelude::*};
use gtk::{gdk, gio, glib};
use std::fs;
use std::hash::{DefaultHasher, Hash, Hasher};
use std::io::Read;
use std::path::PathBuf;
use std::sync::{Arc, Mutex, MutexGuard, RwLock, RwLockReadGuard};
use std::sync::{TryLockError, TryLockResult};
use std::time::{SystemTime, UNIX_EPOCH};

use lofty::file::TaggedFile;
use lofty::prelude::*;
use lofty::probe::Probe;

use crate::library::song_rating::{Ratable, SongRating};
use crate::library::tag_list::{Taggable, Tags};
use crate::library::{Album, SharedAlbum, tag_list};
use crate::util::hint::{cold, unlikely};
use crate::util::{deserialize, serialize, serialize_list, unescaped_split};
use crate::{cache_dir, cold_expression};

pub struct Song {
    album: Mutex<Option<SharedAlbum>>,
    pub path: PathBuf,
    info: RwLock<Option<SongInfo>>,
    user_info: Mutex<UserSongInfo>,
    detailed_info: RwLock<Option<DetailedSongInfo>>,
    thumbnail: RwLock<Option<gdk::Texture>>,
}

pub type SharedSong = Arc<Song>;
pub trait SharedSongExt {
    fn from_path(path: PathBuf) -> SharedSong;
    fn deserialize(data: &str) -> Option<SharedSong>;
}
impl SharedSongExt for SharedSong {
    /// Constructs a new `SharedSong` from a file path
    #[inline]
    fn from_path(path: PathBuf) -> SharedSong {
        Arc::new(Song::from_path(path))
    }
    /// Constructs a new `SharedSong` using serialized data
    /// Returns `Some` on successful load, or `None`
    #[inline]
    fn deserialize(data: &str) -> Option<SharedSong> {
        Song::deserialize(data).map_or_else(|_| None, |song| Some(Arc::new(song)))
    }
}
impl Ratable for SharedSong {
    fn get_rating(&self) -> SongRating {
        self.info().user().rating
    }
    fn set_rating(&self, rating: SongRating) {
        self.info().set_rating(rating);
    }
}
impl Taggable for SharedSong {
    fn get_tags(&self) -> Box<[String]> {
        self.info().user().tags().into()
    }
    fn add_tag(&self, tag: String) {
        self.info().add_tag(
            tag,
            &mut (self.album.lock().unwrap().as_ref())
                .unwrap() // Panic if `album` is not assigned on `self`
                .lock()
                .unwrap(),
        );
    }
    fn remove_tag(&self, tag: &str) {
        self.info().remove_tag(
            tag,
            &mut (self.album.lock().unwrap().as_ref())
                .unwrap() // Panic if `album` is not assigned on `self`
                .lock()
                .unwrap(),
        );
    }
}

#[derive(Debug)]
struct DeserializeSongError;

impl<'s> Song {
    /// Constructs a new `Song` from a file path
    #[inline]
    #[must_use]
    fn from_path(path: PathBuf) -> Song {
        Song {
            album: Mutex::new(None),
            path,
            info: RwLock::new(None),
            user_info: Mutex::new(UserSongInfo::new()),
            detailed_info: RwLock::new(None),
            thumbnail: RwLock::new(None),
        }
    }

    /// Returns a `String` containing serialized song info,
    /// which can be used with the `deserialize()` method
    /// If the song info is not loaded, only the user info
    /// is serialized
    ///
    /// # Panics
    /// Panics if `Path::to_str` conversion fails
    #[inline]
    #[must_use]
    pub fn serialize(&self) -> String {
        let info = self.info();
        let path = self.path.to_str().unwrap();
        let user_info = info.user().clone();
        (info.inspect_basic().as_ref()).map_or_else(
            || {
                serialize! {
                    path => "path",
                    user_info.added => "added",
                    0 => "modified",
                    user_info.play_count => "play_count",
                    user_info.rating => "rating",
                    serialize_list(&user_info.tags) => "tags",
                }
            },
            |info| {
                serialize! {
                    path => "path",
                    info.title => "title",
                    info.album => "album",
                    info.artist => "artist",
                    info.album_artist => "album_artist",
                    info.track => "track",
                    info.disc => "disc",
                    info.year => "year",
                    info.duration_ms => "duration",
                    user_info.added => "added",
                    user_info.modified => "modified",
                    user_info.play_count => "play_count",
                    user_info.rating => "rating",
                    serialize_list(&user_info.tags) => "tags",
                }
            },
        )
    }

    /// Constructs a new `Song` instance using the serialized song info `data`
    ///
    /// # Panics
    /// - If a value cannot be parsed into the required type
    ///
    /// # Errors
    /// - If the `uri` field is missing from the `data`
    #[inline]
    fn deserialize(data: &str) -> Result<Song, DeserializeSongError> {
        let mut path = "";
        let mut info = SongInfo::default();
        let mut user_info = UserSongInfo::default();

        deserialize! {
            data => {
                "path"<str> => path,
                "uri"<str> => path, // COMPAT: Support for loading <= 0.2.2 `songs` file
                "title"<String> => info.title,
                "album"<String> => info.album,
                "artist"<String> => info.artist,
                "album_artist"<String> => info.album_artist,
                "track"<?> => info.track,
                "disc"<?> => info.disc,
                "year"<?> => info.year,
                "duration"<?> => info.duration_ms,
                "added"<?> => user_info.added,
                "modified"<?> => user_info.modified,
                "play_count"<?> => user_info.play_count,
                "rating"<?> => user_info.rating,
                "tags"<[?String]> => user_info.tags,
            }
        }

        if unlikely(path.is_empty()) {
            return Err(DeserializeSongError);
        }

        Ok(Song {
            album: Mutex::new(None),
            path: PathBuf::from(path),
            info: RwLock::new(match user_info.modified {
                0 => cold(None),
                _ => Some(info),
            }),
            user_info: Mutex::new(user_info),
            detailed_info: RwLock::new(None),
            thumbnail: RwLock::new(None),
        })
    }

    /// Returns the song file URI, which can be used by `GStreamer`
    #[inline]
    #[must_use]
    pub fn get_uri(&self) -> glib::GString {
        gio::File::for_path(&self.path).uri()
    }

    /// Returns the assigned album's `MutexGuard`
    /// The value can be `None` if the song is not part of the library.
    ///
    /// # Panics
    /// This function panics if the `album`'s `Mutex` is poisoned
    #[inline]
    pub fn get_album(&self) -> MutexGuard<'_, Option<SharedAlbum>> {
        #[cfg(feature = "lock-warnings")]
        if self.album.try_lock().is_err() {
            eprintln!("Note: Blocking on mutex lock for `Song::get_album`");
        }
        self.album.lock().unwrap()
    }

    /// Sets `self.album` to the given `album`
    ///
    /// # Panics
    /// This function panics if the `album`'s `Mutex` is poisoned
    #[inline]
    pub fn set_album(&self, album: Option<SharedAlbum>) {
        #[cfg(feature = "lock-warnings")]
        if self.album.try_lock().is_err() {
            eprintln!("Note: Blocking on mutex lock for `Song::set_album`");
        }
        *self.album.lock().unwrap() = album;
    }

    /// Checks whether the song is known to the library
    ///
    /// # Panics
    /// The function panics if the `album`'s `Mutex` is poisoned
    #[inline]
    #[must_use]
    pub fn is_within_library(&self) -> bool {
        self.get_album().is_some()
    }

    /// Returns a `SongInfoLoader`, which can be used to access information
    /// about the file and song. Tags are loaded on-demand, and remain in
    /// memory until the respective `unload` or `take` method is called.
    #[inline]
    #[must_use]
    pub const fn info(&'s self) -> SongInfoLoader<'s> {
        SongInfoLoader {
            path: &self.path,
            info: &self.info,
            user_info: &self.user_info,
            detailed_info: &self.detailed_info,
            thumbnail: &self.thumbnail,
            tagged: None,
        }
    }
}

pub struct SongInfoLoader<'i> {
    path: &'i PathBuf,
    info: &'i RwLock<Option<SongInfo>>,
    user_info: &'i Mutex<UserSongInfo>,
    detailed_info: &'i RwLock<Option<DetailedSongInfo>>,
    thumbnail: &'i RwLock<Option<gdk::Texture>>,
    tagged: Option<TaggedFile>,
}

impl SongInfoLoader<'_> {
    /// Whether the two `SongInfoLoader`s are likely to belong to the same `Song`
    ///
    /// Note: if either `SongInfo` is not loaded, equality is determined using the
    /// file path only. For more accurate matching, calling `load_basic` beforehand
    /// might be preferable.
    #[inline]
    #[must_use]
    pub fn matches(&self, other: &SongInfoLoader) -> bool {
        if let (Some(own_info), Some(other_info)) =
            (&*self.inspect_basic(), &*other.inspect_basic())
        {
            own_info == other_info
        } else {
            self.path == other.path
        }
    }

    /// Returns the hash of `self.path`, used for thumbnail files
    #[inline]
    #[must_use]
    pub fn path_hash(&self) -> u64 {
        let mut hasher = DefaultHasher::new();
        self.path.hash(&mut hasher);
        hasher.finish()
    }
    /// Returns this song's thumbnail file path
    #[inline]
    #[must_use]
    pub fn thumbnail_file_path(&self) -> String {
        [cache_dir(), "thumbnails/", &self.path_hash().to_string()].concat()
    }
    /// Returns the song file path
    #[inline]
    #[must_use]
    pub const fn path(&self) -> &PathBuf {
        self.path
    }
    /// Returns the song filename, including the file extestion
    ///
    /// # Panics
    /// The function panics if the filename is not valid UTF-8
    #[inline]
    #[must_use]
    pub fn filename(&self) -> String {
        self.path.file_name().map_or_else(
            || String::from("Unknown"),
            |f| f.to_str().unwrap().to_owned(),
        )
    }
    /// Determines a fallback title using the filename
    #[inline]
    #[must_use]
    fn fallback_title(&self) -> String {
        (self.filename().rsplit_once('.')).map_or(String::new(), |name| name.0.to_owned())
    }
    /// Last known modification time (Unix format); compare with
    /// `file_modification_time()` to detect modifications
    ///
    /// # Panics
    /// The function panics if the user info `Mutex` is poisoned
    #[must_use]
    pub fn known_modification_time(&self) -> u64 {
        #[cfg(feature = "lock-warnings")]
        if self.user_info.try_lock().is_err() {
            eprintln!("Note: Blocking on mutex lock for `known_modification_time`");
        }
        self.user_info.lock().unwrap().modified
    }
    /// Returns the song file modification time, or returns the value
    /// from `fallback` if the modification time is unavailable
    ///
    /// # Panics
    /// Panics if the file modification time is earlier than `UNIX_EPOCH`
    #[inline]
    #[must_use]
    pub fn file_modification_time<F: FnOnce(&Self) -> u64>(&self, fallback: F) -> u64 {
        match self.path.metadata() {
            Ok(info) if let Ok(time) = info.modified() => {
                time.duration_since(UNIX_EPOCH).unwrap().as_secs()
            }
            _ => fallback(self),
        }
    }
    /// Updates the modification time to the current one from the file
    ///
    /// If the file modification time cannot be determined, the current
    /// system time is used instead
    ///
    /// # Panics
    /// The function panics if the user info `Mutex` is poisoned. May
    /// also panic if either the file modification time or system time
    /// is earlier than `UNIX_EPOCH`
    pub fn update_modification_time(&self) {
        #[cfg(feature = "lock-warnings")]
        if self.user_info.try_lock().is_err() {
            eprintln!("Note: Blocking on mutex lock for `update_modification_time`");
        }
        self.user_info.lock().unwrap().modified = self.file_modification_time(|_| {
            (SystemTime::now().duration_since(UNIX_EPOCH).unwrap()).as_secs()
        });
    }

    /// Returns the user song info `MutexGuard`
    ///
    /// # Panics
    /// The function panics if the user info `Mutex` is poisoned
    pub fn user(&self) -> MutexGuard<'_, UserSongInfo> {
        #[cfg(feature = "lock-warnings")]
        if self.user_info.try_lock().is_err() {
            eprintln!("Note: Blocking on mutex lock for `user`");
        }
        self.user_info.lock().unwrap()
    }

    /// Increases the play count by 1
    ///
    /// # Panics
    /// The function panics if the user info `Mutex` is poisoned
    pub fn played(&self) {
        #[cfg(feature = "lock-warnings")]
        if self.user_info.try_lock().is_err() {
            eprintln!("Note: Blocking on mutex lock for `played`");
        }
        self.user_info.lock().unwrap().play_count += 1;
    }

    /// Decreases the play count by 1
    ///
    /// # Panics
    /// The function panics if the user info `Mutex` is poisoned
    pub fn deduct_played(&self) {
        #[cfg(feature = "lock-warnings")]
        if self.user_info.try_lock().is_err() {
            eprintln!("Note: Blocking on mutex lock for `deduct_played`");
        }
        self.user_info.lock().unwrap().play_count -= 1;
    }

    /// Sets the song rating
    ///
    /// # Panics
    /// The function panics if the user info or the album `Mutex` is poisoned
    #[inline]
    pub fn set_rating(&self, rating: SongRating) {
        #[cfg(feature = "lock-warnings")]
        if self.user_info.try_lock().is_err() {
            eprintln!("Note: Blocking on mutex lock for `set_rating`");
        }
        self.user_info.lock().unwrap().rating = rating;
    }

    /// Adds `tag` to the list of user-assigned tags
    /// and updates the album tags and global tag list
    #[inline]
    pub fn add_tag(&mut self, tag: String, album: &mut Album) {
        let mut user_info = self.user();
        if let Err(index) = user_info.tags.find(&tag) {
            tag_list::write_global_tags().add(tag.clone());
            user_info.tags.get_mut().insert(index, tag.clone());
            album.user_info.tags.add(tag);
        }
    }
    /// Removes `tag` from the list of user-assigned tags
    /// and updates the album tags and global tag list
    #[inline]
    pub fn remove_tag(&mut self, tag: &str, album: &mut Album) {
        let mut user_info = self.user();
        if let Ok(index) = user_info.tags.find(tag) {
            tag_list::write_global_tags().remove(tag);
            user_info.tags.get_mut().remove(index);
            album.user_info.tags.remove(tag);
        }
    }

    /// Returns the basic song info if loaded, but does not load it
    ///
    /// Note: This function may block the current thread if the song
    /// info is already being loaded elsewhere; if this is not desired,
    /// use `try_inspect_basic` instead
    ///
    /// # Panics
    /// The function panics if the basic info `RwLock` is poisoned
    #[inline]
    pub fn inspect_basic(&self) -> RwLockReadGuard<'_, Option<SongInfo>> {
        #[cfg(feature = "lock-warnings")]
        if self.info.try_read().is_err() {
            eprintln!(
                "Note: Blocking on read lock for `inspect_basic` (would `try_inspect_basic` make sense here?)"
            );
        }
        self.info.read().unwrap()
    }
    /// Returns the basic song info if accessible without blocking
    /// the current thread, but does not load it
    ///
    /// # Errors
    /// The function errors if the `RwLock` is currently busy
    #[inline]
    pub fn try_inspect_basic(&self) -> TryLockResult<RwLockReadGuard<'_, Option<SongInfo>>> {
        self.info.try_read()
    }
    /// Loads the basic song info unless it is already loaded
    ///
    /// # Panics
    /// The function panics if the basic info `RwLock` is poisoned
    #[inline]
    pub fn load_basic(&mut self) {
        #[cfg(feature = "lock-warnings")]
        if self.info.try_read().is_err() {
            eprintln!(
                "Note: Blocking on read lock for `load_basic` (would `try_load_basic` make sense here?)"
            );
        }
        if self.info.read().unwrap().is_none() {
            self.assign_basic();
        }
    }
    /// Loads the basic song info if needed and runs the given closure
    ///
    /// # Panics
    /// The function panics if the basic info `RwLock` is poisoned
    #[inline]
    pub fn load_basic_and<O, F: FnOnce(&SongInfo) -> O>(&mut self, f: F) -> O {
        #[cfg(feature = "lock-warnings")]
        if self.info.try_read().is_err() {
            eprintln!("Note: Blocking on read lock for `load_basic_and`");
        }
        if let Some(info) = self.info.read().unwrap().as_ref() {
            return f(info);
        }
        self.assign_basic();
        // FIX: Ensure a concurrent unload cannot happen before obtaining the read lock
        match self.info.read().unwrap().as_ref() {
            Some(info) => f(info),
            None => cold_expression! {{
                eprintln!("BUG: Song info is no longer loaded - retrying");
                self.load_basic_and(f)
            }},
        }
    }
    /// Loads the detailed song info if it possible to do so without
    /// blocking the thread and is not already loaded
    ///
    /// # Errors
    /// Returns an error if the info cannot be accessed without blocking
    ///
    /// # Panics
    /// The function panics if the basic info `RwLock` is poisoned
    #[inline]
    pub fn try_load_basic(
        &mut self,
    ) -> Result<(), TryLockError<RwLockReadGuard<'_, Option<SongInfo>>>> {
        if self.info.try_read()?.is_none() {
            self.assign_basic();
        }
        Ok(())
    }
    /// Loads the basic song info and assigns it
    ///
    /// # Panics
    /// The function panics if the detailed info `RwLock` is poisoned
    #[inline]
    fn assign_basic(&mut self) {
        #[cfg(feature = "lock-warnings")]
        if self.detailed_info.try_write().is_err() {
            eprintln!("Note: Blocking on write lock for `assign_basic`");
        }
        let mut info_writer = self.info.write().unwrap();
        // Check if the info was already loaded by another
        // writer while waiting to acquire the write lock
        #[cfg(debug_assertions)]
        if info_writer.is_some() {
            println!(
                "⚠️ Basic song info already loaded (decide whether to include this check it in release builds) ({})",
                line!()
            );
            return;
        }
        *info_writer = Some(self.basic_or_default());
    }
    /// Reads and returns the basic song info from file,
    /// or returns a fallback if unavailable
    #[inline]
    fn basic_or_default(&mut self) -> SongInfo {
        self.load_basic_from_file().unwrap_or_else(|e| {
            eprintln!("Problem loading tags (basic): {:?}: {e}", self.path);
            SongInfo {
                title: self.fallback_title(),
                ..SongInfo::default()
            }
        })
    }
    /// Unloads basic song info and returns the previous value
    ///
    /// # Panics
    /// The function panics if the basic info `RwLock` is poisoned
    #[inline]
    pub fn take_basic(&mut self) -> Option<SongInfo> {
        self.info.write().unwrap().take()
    }
    #[inline]
    fn load_basic_from_file(&mut self) -> Result<SongInfo, Box<dyn Error>> {
        self.update_modification_time();
        if self.tagged.is_none() {
            self.tagged = Some(Probe::read(Probe::open(self.path)?)?);
        }
        // SAFETY: Assigned as `Some` on the previous line
        let tagged = unsafe { self.tagged.as_ref().unwrap_unchecked() };
        let tag = tagged
            .primary_tag()
            .or_else(|| tagged.first_tag())
            .ok_or("No tags found")?;
        let properties = tagged.properties();

        Ok(SongInfo {
            title: tag.title().map_or_else(
                || self.fallback_title(),
                |title| match title.trim().is_empty() {
                    true => self.fallback_title(),
                    false => title.to_string(),
                },
            ),
            album: tag.album().unwrap_or_default().to_string(),
            artist: tag.artist().unwrap_or_default().to_string(),
            album_artist: match tag.get_string(ItemKey::AlbumArtist) {
                Some(album_artist) => album_artist.to_owned(),
                None => tag.artist().unwrap_or_default().to_string(),
            },
            track: tag.track().unwrap_or_default(),
            disc: tag.disk().unwrap_or(1),
            year: tag.date().unwrap_or_default().year,
            #[allow(clippy::cast_possible_truncation)]
            duration_ms: properties.duration().as_millis() as u64,
        })
    }

    /// Returns the detailed song info if loaded, but does not load it
    ///
    /// Note: This function may block the current thread if the song
    /// info is already being loaded elsewhere; if this is not desired,
    /// use `try_inspect_detailed` instead
    ///
    /// # Panics
    /// The function panics if the detailed info `RwLock` is poisoned
    #[inline]
    pub fn inspect_detailed(&self) -> RwLockReadGuard<'_, Option<DetailedSongInfo>> {
        #[cfg(feature = "lock-warnings")]
        if self.detailed_info.try_read().is_err() {
            eprintln!(
                "Note: Blocking on read lock for `inspect_detailed` (would `try_inspect_detailed` make sense here?)"
            );
        }
        self.detailed_info.read().unwrap()
    }
    /// Returns the basic song info if accessible without blocking
    /// the current thread, but does not load it
    ///
    /// # Errors
    /// The function errors if the `RwLock` is currently busy
    #[inline]
    pub fn try_inspect_detailed(
        &self,
    ) -> TryLockResult<RwLockReadGuard<'_, Option<DetailedSongInfo>>> {
        self.detailed_info.try_read()
    }
    /// Loads the detailed song info unless it is alrealy loaded
    ///
    /// # Panics
    /// The function panics if the detailed info `RwLock` is poisoned
    #[inline]
    pub fn load_detailed(&mut self) {
        if self.detailed_info.read().unwrap().is_none() {
            self.assign_detailed();
        }
    }
    /// Loads the detailed song info if needed and runs the given closure
    ///
    /// # Panics
    /// The function panics if the detailed info `RwLock` is poisoned
    #[inline]
    pub fn load_detailed_and<O, F: FnOnce(&DetailedSongInfo) -> O>(&mut self, f: F) -> O {
        #[cfg(feature = "lock-warnings")]
        if self.detailed_info.try_read().is_err() {
            eprintln!("Note: Blocking on read lock for `load_detailed_and`");
        }
        if let Some(info) = self.detailed_info.read().unwrap().as_ref() {
            return f(info);
        }
        self.assign_detailed();
        // FIX: Ensure a concurrent unload cannot happen before obtaining the read lock
        match self.detailed_info.read().unwrap().as_ref() {
            Some(info) => f(info),
            None => cold_expression! {{
                eprintln!("BUG: Detailed song info is no longer loaded - retrying");
                self.load_detailed_and(f)
            }},
        }
    }
    /// Loads the detailed song info and assigns it if it is not already loaded
    ///
    /// # Panics
    /// The function panics if the detailed info `RwLock` is poisoned
    #[inline]
    fn assign_detailed(&mut self) {
        let mut info_writer = self.detailed_info.write().unwrap();
        // Check if the info was already loaded by another
        // writer while waiting to acquire the write lock
        #[cfg(debug_assertions)]
        if info_writer.is_some() {
            println!(
                "⚠️ Detailed song info already loaded (decide whether to include this check it in release builds) ({})",
                line!()
            );
            return;
        }
        *info_writer = Some(self.detailed_or_default());
    }
    /// Attempts to read detailed info from tags and returns it,
    /// or returns a default value if it cannot
    #[inline]
    fn detailed_or_default(&mut self) -> DetailedSongInfo {
        match self
            .tagged_file()
            .map(|tagged| Self::load_tags_detailed(tagged))
        {
            Ok(Ok(result)) => result,
            Err(e) | Ok(Err(e)) => {
                eprintln!("Problem loading tags (detailed): {:?}: {e}", self.path);
                DetailedSongInfo {
                    lyrics: String::new(),
                    artwork: None,
                }
            }
        }
    }
    /// Unloads detailed song info
    ///
    /// # Panics
    /// The function panics if the detailed info `RwLock` is poisoned
    #[inline]
    pub fn unload_detailed(&self) {
        #[cfg(feature = "lock-warnings")]
        if self.detailed_info.try_write().is_err() {
            eprintln!(
                "Note: Blocking on write lock for `unload_detailed` (would `try_unload_detailed` make sense here?)"
            );
        }
        *self.detailed_info.write().unwrap() = None;
    }
    /// Unloads detailed song info if the write lock can be
    /// obtained without blocking, or does nothing otherwise
    #[inline]
    pub fn try_unload_detailed(&self) {
        if let Ok(mut detailed_info) = self.detailed_info.try_write() {
            *detailed_info = None;
        }
    }

    /// Returns a new `TaggedFile` for reading song tags
    #[inline]
    fn tagged_file(&mut self) -> Result<&TaggedFile, Box<dyn Error>> {
        if self.tagged.is_none() {
            self.tagged = Some(Probe::open(self.path)?.read()?);
        }
        // SAFETY: Assigned as `Some` on the previous line
        Ok(unsafe { self.tagged.as_ref().unwrap_unchecked() })
    }

    #[inline]
    fn load_tags_detailed(tagged: &TaggedFile) -> Result<DetailedSongInfo, Box<dyn Error>> {
        // TODO: Would it be possible to cancel artowrk loading while it is in progress?
        let tag = tagged
            .primary_tag()
            .or_else(|| tagged.first_tag())
            .ok_or("No tags found")?;
        Ok(DetailedSongInfo {
            lyrics: tag
                .get_string(ItemKey::Lyrics)
                .unwrap_or_default()
                .to_owned(),
            // TODO: Look for a `cover` file in the song directroy
            // IDEA: Once `cover` files are supported, load both and compare their resolutions
            // and average color delta (to see if they differ) to pick the best one
            // (for average colors, the logic could be factored out from the `settings_page`)
            artwork: if tag.picture_count() > 0 {
                Some(gdk::Texture::from_bytes(&glib::Bytes::from(
                    tag.pictures()[0].data(),
                ))?)
            } else {
                None
            },
        })
    }

    /// Returns the thumbnail without loading it first
    ///
    /// If the returned inner value is `None`, the thumbnail is either
    /// not currently loaded, or it is unavailable for this song
    ///
    /// # Panics
    /// The function panics if the detailed info `RwLock` is poisoned
    #[inline]
    pub fn inspect_thumbnail(&self) -> RwLockReadGuard<'_, Option<gdk::Texture>> {
        #[cfg(feature = "lock-warnings")]
        if self.thumbnail.try_read().is_err() {
            println!(
                "Note: Blocking on read lock for `inspect_thumbnail` (would `try_inspect_thumbnail` make sense here?)"
            );
        }
        self.thumbnail.read().unwrap()
    }
    /// Returns the thumbnail if accessible without blocking the
    /// current thread, but does not load it
    ///
    /// If the returned inner value of the `Ok` variant is `None`,
    /// the thumbnail is either not currently loaded, or it is
    /// unavailable for this song
    ///
    /// # Errors
    /// The function errors if the `RwLock` is currently busy
    #[inline]
    pub fn try_inspect_thumbnail(
        &self,
    ) -> TryLockResult<RwLockReadGuard<'_, Option<gdk::Texture>>> {
        self.thumbnail.try_read()
    }
    /// Loads the thumbnail or creates it if necessary
    ///
    /// Note: The returned inner `Option` could be `None`
    /// if the file does not have an artwork available
    ///
    /// # Panics
    /// The function panics if the detailed info `RwLock` is poisoned
    #[inline]
    pub fn load_thumbnail(&mut self) -> RwLockReadGuard<'_, Option<gdk::Texture>> {
        #[cfg(feature = "lock-warnings")]
        if self.thumbnail.try_read().is_err() {
            println!("Note: Blocking on read lock for `load_thumbnail`");
        }
        let thumbnail = self.thumbnail.read().unwrap();
        if thumbnail.is_some() {
            return thumbnail;
        }
        drop(thumbnail);

        #[cfg(feature = "lock-warnings")]
        if self.thumbnail.try_write().is_err() {
            println!("Note: Blocking on write lock for `load_thumbnail`");
        }
        *self.thumbnail.write().unwrap() = match self.read_thumbnail_from_disk() {
            Ok(thumbnail) => thumbnail,
            Err(_) => self.create_thumbnail(),
        };

        self.thumbnail.read().unwrap()
    }
    /// Unloads the song's thumbnail from memory if it is no longer used
    ///
    /// # Panics
    /// The function panics if the detailed info `RwLock` is poisoned
    #[inline]
    pub fn unload_thumbnail(&mut self) {
        let mut writer = self.thumbnail.write().unwrap();
        if writer.as_ref().is_some_and(|t| t.ref_count() < 2) {
            *writer = None;
        }
    }
    /// Unloads the song's thumbnail from memory if it is no longer used,
    /// but only if possible to do so without blocking
    #[inline]
    pub fn try_unload_thumbnail(&mut self) {
        let Ok(mut writer) = self.thumbnail.try_write() else {
            return;
        };
        if writer.as_ref().is_some_and(|t| t.ref_count() < 2) {
            *writer = None;
        }
    }
    /// Unloads the song's thumbnail from memory and removes it from disk
    ///
    /// # Panics
    /// The function panics if the detailed info `RwLock` is poisoned
    #[inline]
    pub fn invalidate_thumbnail(&mut self) {
        let _ = fs::remove_file(self.thumbnail_file_path());
        *self.thumbnail.write().unwrap() = None;
    }
    /// Reads the song's thumbnail from disk and returns it in the
    /// `Ok(Some)` variant if available. If the thumbnail file could
    /// not be loaded (such as when it is empty), an `Ok(None)` value
    /// is returned.
    ///
    /// # Errors
    /// The function returns an error if the thumbnail file does not exist
    #[inline]
    fn read_thumbnail_from_disk(&self) -> Result<Option<gdk::Texture>, Box<dyn Error>> {
        let mut thumbnail_file = fs::File::open(self.thumbnail_file_path())?;
        let mut buffer = Vec::new();
        thumbnail_file.read_to_end(&mut buffer).unwrap();
        Ok(gdk::Texture::from_bytes(&glib::Bytes::from(&*buffer)).ok())
    }
    /// Creates a new thumbnail file by loading the detailed info
    /// and downscaling it, and returns it as a `gdk::Texture`.
    /// If no artwork is available, a 0-byte thumbnail file is
    /// created, and the function returns `None`.
    #[must_use]
    fn create_thumbnail(&mut self) -> Option<gdk::Texture> {
        let thumbnail_file_path = self.thumbnail_file_path();
        fs::create_dir_all(thumbnail_file_path.rsplit_once('/').unwrap().0).unwrap();

        let Some(artwork) = self.load_detailed_and(|detailed| detailed.artwork.clone()) else {
            fs::write(thumbnail_file_path, "").unwrap();
            return None;
        };

        let mut tex_dl = gdk::TextureDownloader::new(&artwork);
        tex_dl.set_format(gdk::MemoryFormat::R8g8b8a8Premultiplied);
        let (bytes, row_stride) = tex_dl.download_bytes();
        let pixbuf = Pixbuf::from_bytes(
            &bytes,
            gtk::gdk_pixbuf::Colorspace::Rgb,
            true,
            8,
            artwork.width(),
            artwork.height(),
            row_stride as i32,
        )
        .scale_simple(
            256,
            (256.0 / artwork.intrinsic_aspect_ratio()) as i32,
            gtk::gdk_pixbuf::InterpType::Bilinear,
        )
        .unwrap();

        // FIX: `gdk::Texture::for_pixbuf` is deprecated
        // The documentation suggests using `glycin`, however
        // using it might not be feasible for other platforms
        let thumbnail = gdk::Texture::for_pixbuf(&pixbuf);
        thumbnail.save_to_png(thumbnail_file_path).unwrap();

        Some(thumbnail)
    }
}

pub struct SongInfo {
    pub title: String,
    pub album: String,
    pub artist: String,
    pub album_artist: String,
    pub track: u32,
    pub disc: u32,
    pub year: u16,
    pub duration_ms: u64,
}
#[derive(Clone, Debug)]
pub struct UserSongInfo {
    /// Time (in Unix format) when this file was first discovered by the library
    pub added: u64,
    /// Last known modification time (in Unix format).
    /// The maximum value (`!0`) is reserved for new files.
    pub modified: u64,
    /// How many times this song was played
    pub play_count: usize,
    /// User-assigned song rating
    pub rating: SongRating,
    /// User-assigned tags
    tags: Tags,
}
/// Fields which do not need to be held in memory at all times
pub struct DetailedSongInfo {
    pub lyrics: String,
    pub artwork: Option<gdk::Texture>,
}

impl PartialEq for SongInfo {
    #[inline]
    fn eq(&self, other: &Self) -> bool {
        self.title == other.title
            && self.album == other.album
            && self.artist == other.artist
            && self.track == other.track
    }
}
impl Default for SongInfo {
    #[inline]
    fn default() -> Self {
        SongInfo {
            title: String::new(),
            album: String::new(),
            artist: String::new(),
            album_artist: String::new(),
            track: 0,
            disc: 1,
            year: 0,
            duration_ms: 0,
        }
    }
}

impl Default for UserSongInfo {
    /// Returns a default instance of `UserSongInfo`
    ///
    /// This is intended to be used as a placeholder when deserializing
    /// songs. If the file is new to the library, use `new` instead.
    #[inline]
    fn default() -> Self {
        Self {
            added: 0,
            modified: 0,
            play_count: 0,
            rating: SongRating::default(),
            tags: Tags::default(),
        }
    }
}
impl UserSongInfo {
    /// Returns a new instance of `UserSongInfo`
    ///
    /// This is intended to be used when constructing new song entries.
    /// For other usecases, `default` should be used instead.
    #[inline]
    #[must_use]
    pub fn new() -> Self {
        Self {
            added: (SystemTime::now().duration_since(UNIX_EPOCH))
                .map_or_else(|_| 0, |time| time.as_secs()),
            modified: !0,
            ..Self::default()
        }
    }

    /// Returns the list of user-assigned tags for this song
    #[inline]
    #[must_use]
    pub fn tags(&self) -> &[String] {
        &self.tags
    }

    /// Copies info from `other` and merges into `self`:
    /// - Stars are averaged, or whichever one is non-zero is used
    /// - Marks `self` as favorite if either `self` or `other` is favorited
    /// - Play counts are set to the highest number of the two
    /// - Added/modified time is set to the earliest of the two
    /// - Tags missing from `self` are copied from `other`
    #[inline]
    pub fn merge_with(&mut self, other: &UserSongInfo) {
        self.rating.merge_with(&other.rating);
        self.added = self.added.min(other.added);
        self.modified = self.modified.min(other.modified);
        self.play_count = self.play_count.max(other.play_count);
        for tag in &*other.tags {
            if let Err(index) = self.tags.find(tag) {
                self.tags.get_mut().insert(index, tag.to_string());
            }
        }
    }
}
