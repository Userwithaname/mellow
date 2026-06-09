use core::error::Error;
use gdk::{gdk_pixbuf::Pixbuf, prelude::*};
use gtk::{gdk, gio, glib};
use std::fs::{self, File};
use std::hash::{DefaultHasher, Hash, Hasher};
use std::io::Read;
use std::sync::{Arc, Mutex, MutexGuard, RwLock, RwLockReadGuard, RwLockWriteGuard};
use std::sync::{TryLockError, TryLockResult};
use std::time::{SystemTime, UNIX_EPOCH};

use lofty::file::TaggedFile;
use lofty::prelude::*;
use lofty::probe::Probe;

use crate::cache_dir;
use crate::library::SharedAlbum;
use crate::util::{deserialize, serialize, serialize_list, unescaped_split};

pub struct Song {
    album: Mutex<Option<SharedAlbum>>,
    pub file: gio::File,
    pub uri: String,
    info: RwLock<Option<SongInfo>>,
    user_info: Mutex<UserSongInfo>,
    detailed_info: RwLock<Option<DetailedSongInfo>>,
    thumbnail: RwLock<Option<gdk::Texture>>,
}

pub type SharedSong = Arc<Song>;
pub trait SharedSongExt {
    fn from_file(file: gio::File, uri: String) -> SharedSong;
    fn from_path(path: &str) -> SharedSong;
    fn deserialize(data: &str) -> Option<SharedSong>;
    fn album(&self) -> MutexGuard<'_, Option<SharedAlbum>>;
    fn get_album(&self) -> Option<SharedAlbum>;
    fn set_album(&self, album: SharedAlbum);
}
impl SharedSongExt for SharedSong {
    /// Constructs a new `SharedSong` from a `gio::File`
    #[inline]
    fn from_file(file: gio::File, uri: String) -> SharedSong {
        Arc::new(Song::from_file(file, uri))
    }
    /// Constructs a new `SharedSong` from a file path
    #[inline]
    fn from_path(path: &str) -> SharedSong {
        Arc::new(Song::from_path(path))
    }
    /// Constructs a new `SharedSong` using serialized data
    /// Returns `Some` on successful load, or `None`
    #[inline]
    fn deserialize(data: &str) -> Option<SharedSong> {
        Song::deserialize(data).map_or_else(|_| None, |song| Some(Arc::new(song)))
    }
    /// Returns the assigned album's `MutexGuard`
    ///
    /// # Panics
    /// The function panics if the `Mutex` is poisoned
    #[inline]
    fn album(&self) -> MutexGuard<'_, Option<SharedAlbum>> {
        self.album.lock().unwrap()
    }
    /// Returns a cloned reference to the assigned `Option<SharedAlbum>`.
    /// The value can be `None` if the song is not part of the library.
    #[inline]
    fn get_album(&self) -> Option<SharedAlbum> {
        self.album.lock().unwrap().clone()
    }
    /// Sets `self.album` to the given `album`
    #[inline]
    fn set_album(&self, album: SharedAlbum) {
        *self.album.lock().unwrap() = Some(album);
    }
}

#[derive(Debug)]
struct MissingUriError;

impl<'s> Song {
    /// Constructs a new `Song` from a `gio::File`
    #[inline]
    #[must_use]
    fn from_file(file: gio::File, uri: String) -> Song {
        Song {
            album: Mutex::new(None),
            file,
            uri,
            info: RwLock::new(None),
            user_info: Mutex::new(UserSongInfo::new()),
            detailed_info: RwLock::new(None),
            thumbnail: RwLock::new(None),
        }
    }
    /// Constructs a new `Song` from a file path
    #[inline]
    #[must_use]
    fn from_path(file: &str) -> Song {
        let file = gio::File::for_path(file);
        let uri = file.uri().to_string();
        Song {
            album: Mutex::new(None),
            file,
            uri,
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
    #[inline]
    #[must_use]
    pub fn serlialize(&self) -> String {
        let info = self.info();
        let uri = info.file_uri();
        let user_info = info.user().clone();
        (info.inspect_basic().as_ref()).map_or_else(
            || {
                serialize! {
                    uri => "uri",
                    user_info.added => "added",
                    0 => "modified",
                    user_info.play_count => "play_count",
                    user_info.rating => "rating",
                    serialize_list(&user_info.tags) => "tags",
                }
            },
            |info| {
                serialize! {
                    uri => "uri",
                    user_info.added => "added",
                    user_info.modified => "modified",
                    info.title => "title",
                    info.album => "album",
                    info.artist => "artist",
                    info.album_artist => "album_artist",
                    info.track => "track",
                    info.disc => "disc",
                    info.year => "year",
                    info.duration_ms => "duration",
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
    fn deserialize(data: &str) -> Result<Song, MissingUriError> {
        let mut uri = "";
        let mut info = SongInfo::default();
        let mut user_info = UserSongInfo::default();

        deserialize! {
            data => {
                "uri"<str> => uri,
                "added"<?> => user_info.added,
                "modified"<?> => user_info.modified,
                "title"<String> => info.title,
                "album"<String> => info.album,
                "artist"<String> => info.artist,
                "album_artist"<String> => info.album_artist,
                "track"<?> => info.track,
                "disc"<?> => info.disc,
                "year"<?> => info.year,
                "duration"<?> => info.duration_ms,
                "play_count"<?> => user_info.play_count,
                "rating"<?> => user_info.rating,
                "tags"<[String]> => user_info.tags,
            }
        }

        if uri.is_empty() {
            return Err(MissingUriError);
        }

        Ok(Song {
            album: Mutex::new(None),
            file: gio::File::for_uri(uri),
            uri: uri.to_owned(),
            info: RwLock::new(match user_info.modified == 0 {
                false => Some(info),
                true => None,
            }),
            user_info: Mutex::new(user_info),
            detailed_info: RwLock::new(None),
            thumbnail: RwLock::new(None),
        })
    }

    /// Returns a `SongInfoLoader`, which can be used to access information
    /// about the file and song. Tags are loaded on-demand, and remain in
    /// memory until the respective `unload` or `take` method is called.
    #[inline]
    #[must_use]
    pub fn info(&'s self) -> SongInfoLoader<'s> {
        SongInfoLoader {
            file: &self.file,
            uri: &self.uri,
            info: &self.info,
            user_info: &self.user_info,
            detailed_info: &self.detailed_info,
            thumbnail: &self.thumbnail,
            tagged: None,
        }
    }
}

pub struct SongInfoLoader<'i> {
    file: &'i gio::File,
    uri: &'i str,
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
    /// file URIs only. For more accurate matching, calling `load_basic` beforehand
    /// might be preferable.
    #[inline]
    #[must_use]
    pub fn matches(&self, other: &SongInfoLoader) -> bool {
        if let (Some(own_info), Some(other_info)) =
            (&*self.inspect_basic(), &*other.inspect_basic())
        {
            own_info == other_info
        } else {
            self.uri == other.uri
        }
    }

    /// Returns a reference to the `gio::File`
    #[inline]
    #[must_use]
    pub const fn file(&self) -> &gio::File {
        self.file
    }

    /// Retruns the song file URI, which can be used by `GStreamer`
    #[inline]
    #[must_use]
    pub const fn file_uri(&self) -> &str {
        self.uri
    }
    /// Returns the hash of the `file_uri`, used for thumbnail files
    #[inline]
    #[must_use]
    pub fn uri_hash(&self) -> u64 {
        let mut hasher = DefaultHasher::new();
        self.uri.hash(&mut hasher);
        hasher.finish()
    }
    /// Returns this song's thumbnail file path
    #[inline]
    #[must_use]
    pub fn thumbnail_file_path(&self) -> String {
        [cache_dir(), "thumbnails/", &self.uri_hash().to_string()].concat()
    }
    /// Returns the full song file path
    ///
    /// # Panics
    /// The function panics if the path is not valid UTF-8
    #[inline]
    #[must_use]
    pub fn file_path(&self) -> String {
        self.file.path().unwrap().to_str().unwrap().to_owned()
    }
    /// Returns the song filename, including the file extestion
    ///
    /// # Panics
    /// The function panics if the filename is not valid UTF-8
    #[inline]
    #[must_use]
    pub fn filename(&self) -> String {
        self.file.basename().map_or_else(
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
    pub fn known_modification_time(&self) -> i64 {
        self.user_info.lock().unwrap().modified
    }
    /// Returns the song file modification time, or runs the `fallback` closure
    /// and returns its value if the time could not be determined from the file
    #[inline]
    #[must_use]
    pub fn file_modification_time<F: FnOnce(&Self) -> i64>(&self, fallback: F) -> i64 {
        match self.file().query_info(
            gio::FILE_ATTRIBUTE_TIME_MODIFIED,
            gio::FileQueryInfoFlags::empty(),
            gio::Cancellable::NONE,
        ) {
            Ok(file_info) if let Some(time) = file_info.modification_date_time() => time.to_unix(),
            Err(_) | Ok(_) => fallback(self),
        }
    }
    /// Updates the modification time to the current one from the file
    ///
    /// If the file modification time cannot be determined, the current
    /// system time is used instead
    ///
    /// # Panics
    /// The function panics if the user info `Mutex` is poisoned. May
    /// also painc if system time is earlier than `UNIX_EPOCH` and the
    /// file modification time could not be determined.
    pub fn update_modification_time(&self) {
        self.user_info.lock().unwrap().modified = self.file_modification_time(|_| {
            (SystemTime::now().duration_since(UNIX_EPOCH).unwrap()).as_secs() as i64
        });
    }
    /// Sets the known modification time to the provided value
    ///
    /// # Panics
    /// The function panics if the user info `Mutex` is poisoned
    pub fn set_modification_time(&self, time: i64) {
        self.user_info.lock().unwrap().modified = time;
    }

    /// Returns the user song info `MutexGuard`
    ///
    /// # Panics
    /// The function panics if the user info `Mutex` is poisoned
    pub fn user(&self) -> MutexGuard<'_, UserSongInfo> {
        #[cfg(debug_assertions)]
        if self.user_info.try_lock().is_err() {
            eprintln!("Note: Blocking on read lock for `user`");
        }
        self.user_info.lock().unwrap()
    }

    /// Increases the play count by 1
    ///
    /// # Panics
    /// The function panics if the user info `Mutex` is poisoned
    pub fn played(&self) {
        self.user_info.lock().unwrap().play_count += 1;
    }

    /// Decreases the play count by 1
    ///
    /// # Panics
    /// The function panics if the user info `Mutex` is poisoned
    pub fn deduct_played(&self) {
        self.user_info.lock().unwrap().play_count -= 1;
    }

    /// Sets the song rating
    ///
    /// # Panics
    /// The function panics if the user info `RwLock` is poisoned
    pub fn set_rating(&self, rating: u8) {
        self.user_info.lock().unwrap().rating = rating;
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
        #[cfg(debug_assertions)]
        if self.detailed_info.try_read().is_err() {
            eprintln!(
                "Note: Blocking on read lock for `inspect_basic` (would `try_inspect_basic` make sense here?)"
            );
        }
        self.info.read().unwrap()
    }
    /// Returns the basic song info if loaded, but does not load it
    ///
    /// This function blocks until the `RwLock` write lock can be obtained
    ///
    /// # Panics
    /// The function panics if the basic info `RwLock` is poisoned
    #[inline]
    pub fn inspect_basic_mut(&mut self) -> RwLockWriteGuard<'_, Option<SongInfo>> {
        #[cfg(debug_assertions)]
        if self.detailed_info.try_write().is_err() {
            eprintln!("Note: Blocking on write lock for `inspect_basic_mut`");
        }
        self.info.write().unwrap()
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
        #[cfg(debug_assertions)]
        if self.detailed_info.try_write().is_err() {
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
    /// The function panics if the basic info `RwLock` is poisoned. It may
    /// also panic in the rare case where the `SongInfo` is not loaded prior,
    /// and is concurrently unloaded in the brief moment between when the
    /// write guard is dropped and the read guard is obtained.
    #[inline]
    pub fn load_basic_and<O, F: FnOnce(&SongInfo) -> O>(&mut self, f: F) -> O {
        #[cfg(debug_assertions)]
        if self.detailed_info.try_write().is_err() {
            eprintln!("Note: Blocking on read lock for `load_basic_and`");
        }
        if let Some(info) = self.info.read().unwrap().as_ref() {
            return f(info);
        }
        self.assign_basic();
        // FIX: Ensure a concurrent unload cannot happen before obtaining the read lock
        f(self.info.read().unwrap().as_ref().unwrap())
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
        #[cfg(debug_assertions)]
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
            eprintln!(
                "Problem loading tags (basic): {:?}: {e}",
                self.file.path().unwrap_or_default()
            );
            SongInfo {
                title: self.fallback_title(),
                ..SongInfo::default()
            }
        })
    }
    /// Unloads basic song info
    ///
    /// # Panics
    /// The function panics if the basic info `RwLock` is poisoned
    #[inline]
    pub fn unload_basic(&mut self) {
        *self.info.write().unwrap() = None;
    }
    #[inline]
    fn load_basic_from_file(&mut self) -> Result<SongInfo, Box<dyn Error>> {
        self.update_modification_time();
        if self.tagged.is_none() {
            self.tagged = Some(Probe::read(Probe::open(self.file.path().unwrap())?)?);
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
        #[cfg(debug_assertions)]
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
    /// The function panics if the detailed info `RwLock` is poisoned. It may
    /// also panic in the rare case where the `SongInfo` is not loaded prior,
    /// and is concurrently unloaded in the brief moment between when the
    /// write guard is dropped and the read guard is obtained.
    #[inline]
    pub fn load_detailed_and<O, F: FnOnce(&DetailedSongInfo) -> O>(&mut self, f: F) -> O {
        #[cfg(debug_assertions)]
        if self.detailed_info.try_write().is_err() {
            eprintln!("Note: Blocking on read lock for `load_detailed_and`");
        }
        if let Some(info) = self.detailed_info.read().unwrap().as_ref() {
            return f(info);
        }
        self.assign_detailed();
        // FIX: Ensure a concurrent unload cannot happen before obtaining the read lock
        f(self.detailed_info.read().unwrap().as_ref().unwrap())
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
                eprintln!(
                    "Problem loading tags (detailed): {:?}: {e}",
                    self.file.path().unwrap_or_default()
                );
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
        #[cfg(debug_assertions)]
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
            self.tagged = Some(Probe::open(self.file.path().unwrap())?.read()?);
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
        #[cfg(debug_assertions)]
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
        #[cfg(debug_assertions)]
        if self.thumbnail.try_read().is_err() {
            println!("Note: Blocking on read lock for `load_thumbnail`");
        }
        let thumbnail = self.thumbnail.read().unwrap();
        if thumbnail.is_some() {
            return thumbnail;
        }
        drop(thumbnail);

        #[cfg(debug_assertions)]
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
        let mut thumbnail_file = File::open(self.thumbnail_file_path())?;
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
    /// Last known modification time (Unix format). The value -1 is reserved for new files.
    pub modified: i64,
    /// How many times this song was played
    pub play_count: usize,
    /// User-assigned song rating
    pub rating: u8,
    /// User-assigned tags for this song
    pub tags: Vec<String>,
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
            rating: 0,
            tags: Vec::new(),
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
            modified: -1,
            ..Self::default()
        }
    }

    /// Copies info from `other` and merges into `self`:
    /// - Ratings are averaged, or whichever one is non-zero is used
    /// - Play counts are set to the highest number of the two
    /// - Added/modified time is set to the earliest of the two
    #[inline]
    pub fn merge_with(&mut self, other: &UserSongInfo) {
        if self.rating == 0 {
            self.rating = other.rating;
        } else if other.rating > 0 {
            self.rating = (self.rating + other.rating) / 2;
        }
        self.added = self.added.min(other.added);
        self.modified = self.modified.min(other.modified);
        self.play_count = self.play_count.max(other.play_count);
    }
}
