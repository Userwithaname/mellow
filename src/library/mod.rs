use core::hint::cold_path;
use core::sync::atomic::{self, AtomicI8};
use core::{error::Error, mem};
use fastrand;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, MutexGuard, OnceLock, mpsc};
use std::time::{Duration, Instant};
use std::{fs, thread};

pub mod album;
pub mod artist;
pub mod config;
pub mod song;
pub mod song_rating;
pub mod tag_list;
pub mod unload_unused;

pub use album::{Album, SharedAlbum, SortedAlbumSongs};
pub use artist::{Artist, SharedArtist, SortedArtistAlbums};
pub use config::{FILE_SUPPORT, LibraryConfig};
pub use song::{SharedSong, SharedSongExt, Song, SongInfo, SongInfoLoader};

use crate::UI_TIMEOUT;
use crate::excuses::EXP_RX;
use crate::library::song_rating::Ratable;
use crate::library::tag_list::Taggable;
use crate::library::{album::NewSharedAlbum, artist::NewSharedArtist};
use crate::player::{PlayerRequest, QueueItem, player_tx};
use crate::ui::{UpdateUI, ui_tx};
use crate::util::Forever;
use crate::util::tasks::{BoxedTask, Runner};
use crate::util::write_file_create_dir_all;
use crate::{songs_file, util::visit_dirs};

/// Controls and reflects the current library state,
/// using the following constants:
/// - `STATE_CANCEL`: -1
/// - `STATE_READY`: 0
/// - `STATE_BUSY`: 1
pub(super) static STATE: AtomicI8 = AtomicI8::new(STATE_BUSY);
pub(super) const STATE_CANCEL: i8 = -1;
pub(super) const STATE_READY: i8 = 0;
pub(super) const STATE_BUSY: i8 = 1;

pub struct Library {
    songs: Songs,
    albums: Albums,
    artists: Artists,

    missing_songs: Songs,
    check_moved: Forever<Mutex<Songs>>,
    undo_songs: Songs,

    rebuild_pending: bool,
    last_build_started: Instant,

    on_build_succeeded: Vec<LibraryTask>,
    on_build_stopped: Vec<LibraryTask>,
    on_songs_set: Vec<SongsTask>,

    tasks: Runner,
    pub config: LibraryConfig,
    rx: mpsc::Receiver<LibraryRequest>,
}

pub trait RatableAndTaggable: Ratable + Taggable {}
impl<T: Ratable + Taggable> RatableAndTaggable for T {}

pub trait ToQueue {
    fn to_queue(&self) -> Vec<QueueItem>;
}

pub trait ToShuffledQueue {
    fn to_shuffled_queue(&self) -> Vec<QueueItem>;
}

pub type Songs = Vec<Arc<Song>>;
pub trait SortedSongs {
    /// Returns `Ok(index)` if the item was found
    ///
    /// # Errors
    /// If the item was not found, the returned `Err(index)`
    /// can be used to insert the item to the proper position
    fn find_song(&self, path: &Path) -> Result<usize, usize>;
}
impl SortedSongs for Songs {
    #[inline]
    fn find_song(&self, path: &Path) -> Result<usize, usize> {
        self.binary_search_by(|song| (*song.path).cmp(path))
    }
}
impl ToQueue for Songs {
    fn to_queue(&self) -> Vec<QueueItem> {
        self.iter().map(QueueItem::from_song).collect()
    }
}

pub type Albums = Vec<Arc<Mutex<Album>>>;
pub trait SortedAlbums {
    /// Returns `Ok(index)` if the item was found
    ///
    /// # Errors
    /// If the item was not found, the returned `Err(index)`
    /// can be used to insert the item to the proper position
    fn find_album(&self, info: &SongInfo) -> Result<usize, usize>;
}
impl SortedAlbums for Albums {
    #[inline]
    fn find_album(&self, info: &SongInfo) -> Result<usize, usize> {
        self.binary_search_by(|album| {
            let album = album.lock().unwrap();
            (album.artist.lock().unwrap().name.cmp(&info.album_artist))
                .then_with(|| album.title.cmp(&info.album))
        })
    }
}
impl ToQueue for Albums {
    fn to_queue(&self) -> Vec<QueueItem> {
        let mut queue = Vec::<QueueItem>::with_capacity(self.len() * 8);
        for album in self {
            for song in album.lock().unwrap().songs() {
                queue.push(QueueItem::Song(Arc::clone(song)));
            }
        }
        queue
    }
}
impl ToShuffledQueue for Albums {
    fn to_shuffled_queue(&self) -> Vec<QueueItem> {
        let mut queue = Vec::with_capacity(self.len() * 8);
        let mut shuffled: Vec<usize> = (0..self.len()).collect();
        for i in 0..shuffled.len() {
            let rand_index = fastrand::usize(0..shuffled.len());
            shuffled.swap(i, rand_index);
        }
        for index in shuffled {
            for song in self[index].lock().unwrap().songs() {
                queue.push(QueueItem::Song(Arc::clone(song)));
            }
        }
        queue
    }
}

pub type Artists = Vec<Arc<Mutex<Artist>>>;
pub trait SortedArtists {
    /// Returns `Ok(index)` if the item was found
    ///
    /// # Errors
    /// If the item was not found, the returned `Err(index)`
    /// can be used to insert the item to the proper position
    fn find_artist(&self, info: &SongInfo) -> Result<usize, usize>;
    /// Returns an `Option<SharedSong>` depending on whether the song
    /// was found within the library or not
    ///
    /// # Panics
    /// Panics if the artist or album candidate `Mutex` is in a poisoned state
    fn locate_song_by_info(&self, info: &SongInfo) -> Option<SharedSong>;
}
impl SortedArtists for Artists {
    #[inline]
    fn find_artist(&self, info: &SongInfo) -> Result<usize, usize> {
        self.binary_search_by(|artist| artist.lock().unwrap().name.cmp(&info.album_artist))
    }

    #[inline]
    fn locate_song_by_info(&self, info: &SongInfo) -> Option<SharedSong> {
        if info.title.is_empty() {
            return None;
        }

        let artist = match self.find_artist(info) {
            // SAFETY: `Ok` variant returned by `find_artist` is always within bounds
            Ok(artist_index) => unsafe { self.get_unchecked(artist_index).lock().unwrap() },
            Err(_) => return None,
        };

        let albums = artist.albums();
        let album = match albums.find_artist_album(info) {
            // SAFETY: `Ok` variant returned by `find_artist_album` is always within bounds
            Ok(album_index) => unsafe { albums.get_unchecked(album_index).lock().unwrap() },
            Err(_) => return None,
        };

        let songs = album.songs();
        songs.find_album_song(info).ok().map(|song_index| {
            // SAFETY: `Ok` variant returned by `find_album_song` is always within bounds
            unsafe { Arc::clone(songs.get_unchecked(song_index)) }
        })
    }
}
impl ToQueue for Artists {
    fn to_queue(&self) -> Vec<QueueItem> {
        let mut queue = Vec::<QueueItem>::with_capacity(self.len() * 16);
        for artist in self {
            for album in artist.lock().unwrap().albums() {
                for song in album.lock().unwrap().songs() {
                    queue.push(QueueItem::Song(Arc::clone(song)));
                }
            }
        }
        queue
    }
}
impl ToShuffledQueue for Artists {
    fn to_shuffled_queue(&self) -> Vec<QueueItem> {
        let mut queue = Vec::with_capacity(self.len() * 16);
        let mut shuffled: Vec<usize> = (0..self.len()).collect();
        for i in 0..shuffled.len() {
            let rand_index = fastrand::usize(0..shuffled.len());
            shuffled.swap(i, rand_index);
        }
        for index in shuffled {
            for album in self[index].lock().unwrap().albums() {
                for song in album.lock().unwrap().songs() {
                    queue.push(QueueItem::Song(Arc::clone(song)));
                }
            }
        }
        queue
    }
}

static LIBRARY_TX: OnceLock<mpsc::Sender<LibraryRequest>> = OnceLock::new();
/// Returns the channel sender for sending requests to the library using `LibraryRequest`
///
/// # Safety
/// Causes undefined behavior if called before `init_channels`
#[inline]
#[must_use]
pub fn library_tx() -> &'static mpsc::Sender<LibraryRequest> {
    // SAFETY: `init_channels` runs in `Application::run`, before starting any threads
    unsafe { LIBRARY_TX.get().unwrap_unchecked() }
}
/// Initializes the library channel sender accessed through `library_tx()`
///
/// # Errors
/// The function returns an error if `LIBRARY_TX` has already been initialized
#[inline]
pub fn init_library_tx(
    library_tx: mpsc::Sender<LibraryRequest>,
) -> Result<(), mpsc::Sender<LibraryRequest>> {
    LIBRARY_TX.set(library_tx)
}

type LibraryTask = Box<dyn FnOnce(&mut Library) + Send + 'static>;
type SongsTask = Box<dyn FnOnce(&Songs) + Send + 'static>;

pub enum LibraryRequest {
    /// Rebuilds the library, cancelling any rebuild that might be currently running
    /// For more responsive cancellation, use `Library::rebuild` instead
    Rebuild(Instant),
    /// Cancels the current library build and pauses the thread pool requests
    /// until all current tasks finish running
    CancelRebuild(Instant),

    /// Starts a player queue using the given file or directory paths
    QueueFromPaths(Vec<PathBuf>),

    /// Adds a new library directory to the configuration
    AddLibrary(PathBuf),
    /// Removes the library directory at the given index from the configuration
    RemoveLibrary(usize),

    /// Remembers the removed directory for undo
    RegisterUndoDirectory(PathBuf),
    /// Re-adds the removed directory and restores its library data
    UndoRemovedDirectory(PathBuf),

    /// Runs the given task on the thread pool, in the background
    RunTask(BoxedTask),
    /// Runs the given task on the library thread directly, with mutable access to the `Library`
    RunLibraryTask(LibraryTask),

    /// Renames `tag` to `new_tag` on all songs in the library (including missing ones),
    /// and notifies when finished through `notify_done`
    RenameTag {
        tag: String,
        new_name: String,
        notify_done: mpsc::Sender<()>,
    },

    /// Cleanly shuts down the library and thread pool, and writes the configuration data to disk
    Uninit,
}

impl Library {
    /// Constructs a new instance of `Library`
    #[inline]
    #[must_use]
    pub fn init(config: LibraryConfig, library_rx: mpsc::Receiver<LibraryRequest>) -> Library {
        Library {
            songs: Library::deserialize_songs(),
            albums: Vec::new(),
            artists: Vec::new(),

            missing_songs: Vec::new(),
            check_moved: Forever::new(Mutex::new(Vec::new())),
            undo_songs: Vec::new(),

            rebuild_pending: false,
            last_build_started: Instant::now(),

            on_build_succeeded: Vec::new(),
            on_build_stopped: Vec::new(),
            on_songs_set: Vec::new(),

            tasks: Runner::new(
                thread::available_parallelism()
                    .map_or(4, |cores| usize::from(cores).saturating_sub(4).max(4)),
            ),
            // IDEA: Maybe there could be a power-saver option?
            // tasks: Runner::new(4),
            config,
            rx: library_rx,
        }
    }

    /// Returns `true` if there are no songs in the library
    /// (otherwise returns `false`)
    #[inline]
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.songs.is_empty()
    }

    /// Main loop for handling library requests
    ///
    /// # Errors
    /// The function may error upon handling a request,
    /// in most cases due to a closed channel receiver
    ///
    /// # Panics
    /// The function may panic upon handling a request if
    /// a poisoned `Mutex` is passed. When processing
    /// `LibraryRequest::QueueFromPaths`, the function
    /// panics if any path contains invalid unicode.
    #[inline]
    pub fn request_handler(mut self) -> Result<(), Box<dyn Error>> {
        loop {
            match self.rx.recv()? {
                LibraryRequest::RunTask(task) => self.tasks.run(task),
                LibraryRequest::RunLibraryTask(f) => f(&mut self),

                LibraryRequest::QueueFromPaths(paths) => self.play_from_paths(
                    paths.iter().map(|path| path.to_str().unwrap()), //
                )?,

                LibraryRequest::CancelRebuild(time) => self.cancel_library_build(time),
                LibraryRequest::Rebuild(time) => self.start_library_build(time),

                LibraryRequest::AddLibrary(dir) => self.config.add_library(dir),
                LibraryRequest::RemoveLibrary(index) => self.config.remove_library(index),

                LibraryRequest::RegisterUndoDirectory(dir) => {
                    self.register_undo_directory(&dir);
                }
                LibraryRequest::UndoRemovedDirectory(dir) => self.undo_removed_directory(dir),

                LibraryRequest::RenameTag {
                    tag,
                    new_name,
                    notify_done,
                } => self.rename_tag(&tag, &new_name, &notify_done),

                #[allow(clippy::unit_arg)]
                LibraryRequest::Uninit => return Ok(self.shutdown()),
            }
        }
    }

    /// Locates song files within the configured directories, then
    /// runs `create_connections()` as a background task. Existing
    /// song entries are preserved, and only new songs are added.
    /// Missing song entries are handled in the background task.
    ///
    /// # Note
    /// Either `STATE` should be set to `STATE_BUSY` before calling,
    /// or `start_library_build()` should be called instead
    ///
    /// # Panics
    /// The function panics if the library channel is closed
    pub fn discover_files(&mut self) {
        debug_assert!(
            STATE.load(atomic::Ordering::Acquire) == STATE_BUSY,
            "`STATE` should be set to `STATE_BUSY` ({STATE_BUSY}) before calling `discover_files`"
        );

        self.rebuild_pending = false;
        for library_path in self.config.directories() {
            let _ = visit_dirs(library_path.to_path_buf(), &mut |path| {
                // Add the song to the library if it is new, and the extension is supported
                if path.extension().is_some_and(extension_supported)
                    && let Err(index) = self.songs.find_song(&path)
                {
                    self.songs.insert(index, SharedSong::from_path(path));
                }
            })
            .inspect_err(|e| eprintln!("Error reading '{library_path:?}': {e}"));
        }

        if STATE.load(atomic::Ordering::Acquire) == STATE_CANCEL {
            return;
        }

        #[cfg(feature = "verbose-logs")]
        println!(
            "Library song file list built in {:?}",
            self.last_build_started.elapsed()
        );

        self.tasks.run({
            let songs = self.songs.clone();
            let missing = mem::take(&mut self.missing_songs);
            let check = self.check_moved.static_ref();
            let config = self.config.clone();
            let n_workers = self.tasks.num_workers();
            move || match Library::create_connections(songs, missing, check, &config, n_workers) {
                Ok(()) => (),
                Err(e) => eprintln!("`Library::create_connections`: {e}"),
            }
        });
    }

    /// Creates connections between library `songs`/`albums`/`artists`
    ///
    /// # Errors
    /// Returns an error if the build was cancelled, or if the library or UI
    /// channel receiver is closed
    ///
    /// # Panics
    /// The function panics if `songs`, `missing`, or `check_moved` contains a
    /// poisoned mutex
    #[inline]
    fn create_connections(
        mut songs: Songs,
        mut missing: Songs,
        check_moved: &Mutex<Songs>,
        config: &LibraryConfig,
        num_workers: usize,
    ) -> Result<(), Box<dyn Error>> {
        Library::validate_songs(&mut songs, &mut missing, check_moved, config);

        let ui_tx = ui_tx();
        let library_tx = library_tx();

        let times_task = Arc::new(Mutex::new(()));
        let _ = library_tx.send(LibraryRequest::RunLibraryTask(Box::new({
            let songs = songs.clone();
            let times_task = Arc::clone(&times_task);
            move |library| {
                library.set_missing_songs(missing);
                library.set_songs(songs.clone());

                #[cfg(feature = "verbose-logs")]
                println!(
                    "Library songs validated in {:?}",
                    library.last_build_started.elapsed()
                );

                // Check file modification times and start song info loading tasks
                Library::run_task(
                    library_tx,
                    Box::new(move || {
                        let times_task_lock = times_task.lock();
                        Library::check_modification_times(&songs);
                        drop(times_task_lock);

                        thread::sleep(Duration::from_millis(100));
                        if STATE.load(atomic::Ordering::Relaxed) == STATE_BUSY {
                            Library::load_info_in_background(songs, num_workers - 2);
                        }
                    }),
                );
            }
        })));

        let mut albums = Vec::with_capacity(songs.len() / 16);
        let mut artists = Vec::with_capacity(songs.len() / 64);

        let mut timer = Instant::now() - UI_TIMEOUT;
        let progress_step = 1.0 / songs.len() as f64;
        let mut progress = 0.0;

        for song in &songs {
            if timer.elapsed() > UI_TIMEOUT {
                timer = Instant::now();
                if STATE.load(atomic::Ordering::Relaxed) == STATE_CANCEL {
                    let _ = ui_tx.send_blocking(UpdateUI::Progress(None));
                    break;
                }
                let _ = ui_tx.send_blocking(UpdateUI::Progress(Some(progress)));
            }

            song.info().load_basic_and(|song_info| {
                let album_index = albums.find_album(song_info);
                let artist_index = artists.find_artist(song_info);

                match artist_index {
                    Ok(artist_index) => match album_index {
                        Ok(album_index) => {
                            // SAFETY: `album_index` is `Ok`, therefore within bounds
                            let album = unsafe { albums.get_unchecked(album_index) };
                            let mut album_locked = album.lock().unwrap();

                            // Add the song to the album songs
                            album_locked.add_song(Arc::clone(song), song_info);
                            drop(album_locked);

                            // Associate the song with its album
                            song.set_album(Some(Arc::clone(album)));
                        }
                        Err(album_index) => {
                            // SAFETY: `artist_index` is `Ok`, therefore within bounds
                            let artist = unsafe { artists.get_unchecked(artist_index) };
                            let album = SharedAlbum::new_album(
                                song_info, //
                                Arc::clone(song),
                                Arc::clone(artist),
                            );

                            // Add the album to the artist's albums
                            let mut artist_locked = artist.lock().unwrap();
                            artist_locked.add_album(Arc::clone(&album), song_info);
                            drop(artist_locked);

                            // Add to the library albums as well
                            albums.insert(album_index, Arc::clone(&album));

                            // Associate the song with its album
                            song.set_album(Some(album));
                        }
                    },
                    Err(artist_index) => {
                        // Create the artist and album connected pair
                        let (artist, album) = SharedArtist::new_artist_album_pair(
                            song_info, //
                            Arc::clone(song),
                        );

                        // Add to the library albums as well
                        match album_index {
                            Err(index) | Ok(index) => albums.insert(index, Arc::clone(&album)),
                        }

                        // Add the artist entry
                        artists.insert(artist_index, artist);

                        // Associate the song with its album
                        song.set_album(Some(album));
                    }
                }
            });

            progress += progress_step;
        }

        #[cfg(feature = "verbose-logs")]
        println!("Library connections have finished building");

        if STATE.load(atomic::Ordering::Acquire) != STATE_CANCEL {
            if let Ok(check_moved) = check_moved.lock()
                && !check_moved.is_empty()
            {
                Library::merge_moved_entries(&artists, check_moved);
            }
            (ui_tx.send_blocking(UpdateUI::SetLibrarySongs(songs))).expect(EXP_RX);
        }

        #[cfg(feature = "verbose-logs")]
        println!("Merged moved file entries");

        let state = STATE.load(atomic::Ordering::Acquire);
        library_tx.send(LibraryRequest::RunLibraryTask(Box::new(move |library| {
            if state != STATE_CANCEL {
                library.set_artists(artists);
                library.set_albums(albums);
                // Songs were already set before checking modification times

                // Correct paths in the queue if any have changed
                (player_tx().send(PlayerRequest::ValidateFilePaths)).expect(EXP_RX);
            }

            // Wait for the modification times check to finish
            drop(times_task.lock().unwrap());

            // Cancel any remaining background tasks
            library.cancel_library_build(Instant::now());

            if state != STATE_CANCEL {
                // IDEA: Maybe a negative progress value could represent a failure state?
                ui_tx.send_blocking(UpdateUI::Progress(None)).expect(EXP_RX);

                #[cfg(feature = "verbose-logs")]
                println!(
                    "Library connections finished in {:?}",
                    library.last_build_started.elapsed()
                );

                library.build_succeeded();
            }

            // Wait for all tasks to stop before calling `build_stopped`
            if STATE.load(atomic::Ordering::Acquire) != STATE_READY {
                library.cancel_library_build_blocking(Instant::now());
            }

            library.build_stopped();
        })))?;

        match state {
            STATE_BUSY => Ok(()),
            _ => Err(format!("Cancelled (`STATE`: {state})"))?,
        }
    }

    /// Checks the file modification times and requests a rebuild if any of them have changed
    #[inline]
    fn check_modification_times(songs: &Songs) {
        #[cfg(feature = "verbose-logs")]
        println!("Checking modification times");

        let mut needs_rebuild = false;

        for song in songs {
            if STATE.load(atomic::Ordering::Relaxed) == STATE_CANCEL {
                return;
            }

            let mut info = song.info();
            let known_modification_time = info.known_modification_time();
            if known_modification_time == !0
                || known_modification_time
                    == info.file_modification_time(|info| {
                        eprintln!(
                            "WARNING: Modification time could not be read: '{:?}'; skipping...",
                            info.path()
                        );
                        known_modification_time
                    })
            {
                continue;
            }

            needs_rebuild |= info.take_basic().is_some();
            info.invalidate_thumbnail();
        }

        // If files were modified, queue another rebuild so the new info gets loaded
        if needs_rebuild && STATE.swap(STATE_CANCEL, atomic::Ordering::Release) != STATE_CANCEL {
            let _ = library_tx().send(LibraryRequest::RunLibraryTask(Box::new(|library| {
                library.cancel_library_build_blocking(Instant::now());
                (ui_tx().send_blocking(UpdateUI::Progress(Some(0.0)))).expect(EXP_RX);
                println!("Rebuilding because files were modified");
                library.start_library_build(Instant::now());
            })));
            println!("Modifications detected, library will rebuild shortly");
        } else {
            #[cfg(feature = "verbose-logs")]
            println!("Modification times were checked - nothing to do");
        }
    }

    /// Loads song info from `songs` in the background by even distributing them
    /// among worker tasks which run on the thread pool. The number of tasks is
    /// determined by `num_tasks`.
    #[inline]
    fn load_info_in_background(songs: Songs, num_tasks: usize) {
        let mut worker_songs = (0..num_tasks)
            .map(|_| Vec::<SharedSong>::with_capacity(songs.len() / num_tasks))
            .collect::<Vec<Vec<SharedSong>>>();
        let mut target_worker = 0;
        for song in songs {
            worker_songs[target_worker].push(song);
            target_worker += 1;
            if target_worker == num_tasks {
                target_worker = 0;
            }
        }

        #[cfg(feature = "verbose-logs")]
        println!("Starting {num_tasks} background tasks to load song info");

        for songs in worker_songs {
            Library::run_task(library_tx(), move || {
                for song in songs {
                    if STATE.load(atomic::Ordering::Relaxed) != STATE_BUSY {
                        #[cfg(feature = "verbose-logs")]
                        println!("Song info task was cancelled");
                        return;
                    }
                    drop(song.info().try_load_basic());
                }
            });
        }
    }

    /// Ensures validity of the provided `songs`:
    /// - Sorts `songs` and resolves duplicate entries
    /// - Moves missing files from `songs` into `missing_songs`
    /// - Removes and returns a list of `songs` whose files may
    ///   have been moved on disk
    /// - Updates the library `songs` and `missing_songs`
    ///
    /// # Panics
    /// The function may panic if the library channel is closed
    /// or if a song's `Mutex` is in a poisoned state
    #[inline]
    fn validate_songs(
        songs: &mut Songs,
        missing: &mut Songs,
        check_moved: &Mutex<Songs>,
        config: &LibraryConfig,
    ) {
        let mut libraries = Vec::with_capacity(config.directories().len());
        let mut missing_libraries = Vec::new();
        for (index, dir) in config.directories().iter().enumerate() {
            match fs::exists(&config.directories()[index]) {
                Ok(true) => libraries.push(dir),
                _ => missing_libraries.push(dir),
            }
        }

        let mut check_moved = check_moved.lock().unwrap();
        let mut old_songs = [
            mem::replace(songs, Vec::with_capacity(songs.len())),
            mem::take(&mut *check_moved),
            mem::take(missing),
        ]
        .concat()
        .into_iter();

        while let Some(song) = old_songs.next() {
            if !song.path.extension().is_some_and(extension_supported) {
                cold_path();
                continue;
            }

            match songs.find_song(&song.path) {
                // Valid song entry
                Err(index) if fs::exists(&song.path).unwrap_or_default() => {
                    // Filter songs outside of `config.directories`
                    if libraries.iter().any(|dir| song.path.starts_with(dir)) {
                        songs.insert(index, song);
                        continue;
                    }
                    // IDEA: To disable libraries, move `songs` into `disabled_songs`

                    // The file may have been copied to an active library
                    check_moved.push(song);
                }
                // Missing file
                Err(_) => {
                    match missing.find_song(&song.path) {
                        // New missing song entry
                        Err(index) => {
                            // Only remember missing files if they are within
                            // a library directory which is currently missing
                            // (otherwise, they were either moved or removed)
                            if (missing_libraries.iter()).any(|dir| song.path.starts_with(dir)) {
                                #[cfg(feature = "verbose-logs")]
                                println!(
                                    "Remembering {:?} because its library is missing",
                                    song.path
                                );
                                missing.insert(index, song);
                                continue;
                            }

                            check_moved.push(song);
                        }
                        // Duplicate missing song entry
                        Ok(index) => {
                            let missing = &missing[index];
                            if !Arc::ptr_eq(&song, missing) {
                                song.info().user().merge_with(&missing.info().user());
                            }
                            drop(song);
                        }
                    }
                }
                // Duplicate entry
                Ok(index) => {
                    #[cfg(feature = "verbose-logs")]
                    println!("Resolving duplicate entry: {:?}", song.path);

                    // SAFETY: `index` is `Ok`, therefore within bounds
                    let existing = unsafe { songs.get_unchecked(index) };
                    if !Arc::ptr_eq(&song, existing) {
                        existing.info().user().merge_with(&song.info().user());
                    }

                    drop(song);
                }
            }

            if STATE.load(atomic::Ordering::Relaxed) == STATE_CANCEL {
                check_moved.extend(&mut old_songs);
                return;
            }
        }
    }

    /// Attempts to locate missing files if they were moved and merges
    /// them with the existing song entries so their info is preserved
    ///
    /// # Panics
    /// The function panics if the UI channel receiver is unititialized
    /// or closed, or if the `check_moved` mutex is in a poisoned state
    fn merge_moved_entries(artists: &Artists, mut check_moved: MutexGuard<'_, Songs>) {
        let ui_tx = ui_tx();
        let progress_step = 1.0 / check_moved.len() as f64;
        let mut progress = 0.0;
        let mut timer = Instant::now();

        while let Some(missing) = check_moved.pop() {
            let mut old_info = missing.info();
            match old_info.load_basic_and(|info| artists.locate_song_by_info(info)) {
                Some(found_entry) => {
                    // Copy the user-assigned song info to the new entry
                    let found_entry_info = found_entry.info();
                    found_entry_info.user().merge_with(&old_info.user());

                    // Rename the thumbnail file
                    let _ = fs::rename(
                        old_info.thumbnail_file_path(),
                        found_entry_info.thumbnail_file_path(),
                    );

                    #[cfg(feature = "verbose-logs")]
                    println!(
                        "Found moved file:\n{:?} -> {:?}",
                        old_info.path(),
                        found_entry_info.path()
                    );
                }
                None => {
                    let _ = fs::remove_file(old_info.thumbnail_file_path());
                }
            }

            progress += progress_step;
            if timer.elapsed() > UI_TIMEOUT {
                timer = Instant::now();
                if STATE.load(atomic::Ordering::Relaxed) == STATE_CANCEL {
                    return;
                }
                let _ = ui_tx.send_blocking(UpdateUI::Progress(Some(progress)));
            }
        }
    }

    /// Cancels any currently ongoing rebuild and requests a new one
    ///
    /// # Panics
    /// Panics if the library channel is closed
    pub fn rebuild() {
        if STATE.load(atomic::Ordering::Acquire) == STATE_BUSY {
            STATE.store(STATE_CANCEL, atomic::Ordering::Relaxed);
            library_tx()
                .send(LibraryRequest::CancelRebuild(Instant::now()))
                .expect(EXP_RX);
        }
        library_tx()
            .send(LibraryRequest::Rebuild(Instant::now()))
            .expect(EXP_RX);
    }

    /// Starts a new library build
    ///
    /// If already building, the current operation is cancelled
    /// before starting a new one
    pub fn start_library_build(&mut self, requested_at: Instant) {
        if requested_at < self.last_build_started {
            #[cfg(feature = "verbose-logs")]
            println!("Rebuild request timed out; skipping");

            return;
        }

        match STATE.compare_exchange(
            STATE_READY,
            STATE_BUSY,
            atomic::Ordering::Acquire,
            atomic::Ordering::Relaxed,
        ) {
            Ok(_) => {
                #[cfg(feature = "verbose-logs")]
                println!("Starting rebuild");

                self.last_build_started = Instant::now();
                self.discover_files();
            }
            Err(_) => {
                if self.rebuild_pending {
                    #[cfg(feature = "verbose-logs")]
                    println!("Rebuild already queued; ignoring request");

                    return; // Skip duplicate pending rebuild requests
                }
                self.rebuild_pending = true;
                self.cancel_library_build(Instant::now());

                #[cfg(feature = "verbose-logs")]
                println!("Rebuilding when ready");

                self.run_on_build_stopped(Box::new(move |library| {
                    if requested_at > library.last_build_started {
                        STATE.store(STATE_BUSY, atomic::Ordering::Release);

                        #[cfg(feature = "verbose-logs")]
                        println!("Rebuilding now");

                        library.last_build_started = Instant::now();
                        library.discover_files();
                    }
                }));
            }
        }
    }

    /// Cancels any currently running library build operation
    #[inline]
    pub fn cancel_library_build(&self, requested_at: Instant) {
        if requested_at < self.last_build_started {
            #[cfg(feature = "verbose-logs")]
            println!("Cancellation request timed out; skipping");

            return;
        }

        STATE.swap(STATE_CANCEL, atomic::Ordering::Release);
        self.tasks.await_all_tasks();
    }

    /// Cancels any currently running library build operation
    /// and blocks the current thread until fully cancelled
    #[inline]
    pub fn cancel_library_build_blocking(&self, requested_at: Instant) {
        if requested_at < self.last_build_started {
            #[cfg(feature = "verbose-logs")]
            println!("Cancellation request timed out; skipping");

            return;
        }

        STATE.store(STATE_CANCEL, atomic::Ordering::Release);
        self.tasks.await_all_tasks();

        let library_thread = thread::current();
        self.tasks.run(move || library_thread.unpark());
        // Parking the thread in a loop until cancellation, because
        // threads can supposedly unpark themselves in some cases
        while STATE.load(atomic::Ordering::Acquire) == STATE_CANCEL {
            thread::park();
        }
    }

    /// Uses `library_tx` to send the `task` to run on the thread pool.
    /// If idle threads are available, the `task` will run when the
    /// library processes the request, otherwise, it will wait in a queue.
    #[inline]
    pub fn run_task<T>(library_tx: &mpsc::Sender<LibraryRequest>, task: T)
    where
        T: FnOnce() + Into<Box<T>> + Send + 'static,
    {
        if let Err(e) = library_tx.send(LibraryRequest::RunTask(task.into())) {
            eprintln!("Could not run task: {e}");
        }
    }

    /// Returns all songs known to the library
    #[inline]
    #[must_use]
    pub const fn songs(&self) -> &Songs {
        &self.songs
    }
    /// Replaces `self.songs` with `songs`
    #[inline]
    fn set_songs(&mut self, songs: Songs) {
        for f in mem::take(&mut self.on_songs_set) {
            f(&songs);
        }
        self.songs = songs;
    }
    /// Runs the given task the next time library songs are updated
    #[inline]
    pub fn run_on_songs_set(&mut self, f: SongsTask) {
        self.on_songs_set.push(f);
    }

    /// Returns all albums known to the library
    #[inline]
    #[must_use]
    pub const fn albums(&self) -> &Albums {
        &self.albums
    }
    /// Replaces `self.albums` with `albums`, and updates the library albums UI
    ///
    /// # Panics
    /// The function panics if the UI channel receiver is closed
    #[inline]
    fn set_albums(&mut self, albums: Albums) {
        (ui_tx().send_blocking(UpdateUI::SetLibraryAlbums(albums.clone()))).expect(EXP_RX);
        self.albums = albums;
    }

    /// Returns all artists known to the library
    #[inline]
    #[must_use]
    pub const fn artists(&self) -> &Artists {
        &self.artists
    }
    /// Replaces `self.artists` with `artists`, and updates the library artists UI
    ///
    /// # Panics
    /// The function panics if the UI channel receiver is closed
    #[inline]
    fn set_artists(&mut self, artists: Artists) {
        (ui_tx().send_blocking(UpdateUI::SetLibraryArtists(artists.clone()))).expect(EXP_RX);
        self.artists = artists;
    }
    /// Replaces `self.missing_songs` with `missing_songs`
    #[inline]
    fn set_missing_songs(&mut self, missing_songs: Songs) {
        self.missing_songs = missing_songs;
    }

    /// Runs the given task once the library build is done,
    /// regardless if it succeeded or failed. If not building,
    /// the task will run right away.
    #[inline]
    pub fn run_on_build_stopped(&mut self, f: LibraryTask) {
        match STATE.load(atomic::Ordering::Acquire) {
            STATE_READY => f(self),
            STATE_CANCEL => {
                self.cancel_library_build_blocking(Instant::now());
                f(self); // Run directly once fully cancelled
            }
            _ => self.on_build_stopped.push(f),
        }
    }
    /// Runs the tasks in `on_build_stopped` and leaves it empty
    ///
    /// Call this function once the library build is done, regardless
    /// of whether it succeeded or failed
    #[inline]
    fn build_stopped(&mut self) {
        for f in mem::take(&mut self.on_build_stopped) {
            f(self);
        }
    }

    /// Runs the given task once the library build successfully completes in full
    #[inline]
    pub fn run_on_build_succeeded(&mut self, f: LibraryTask) {
        self.on_build_succeeded.push(f);
    }
    /// Runs the tasks in `on_build_succeeded` and leaves it empty
    ///
    /// Call this function once the library build has succeeded
    #[inline]
    fn build_succeeded(&mut self) {
        for f in mem::take(&mut self.on_build_succeeded) {
            f(self);
        }
    }

    /// Adds all songs from directory `dir` to `self.undo_songs`, so their
    /// info can be recovered using `LibraryRequest::UndoRemovedDirectory`
    fn register_undo_directory(&mut self, dir: &PathBuf) {
        let Err(start_index) = self.songs.find_song(dir) else {
            unreachable!( /* `dir` is a directory, not a song file */ )
        };
        for song in self.songs.iter().skip(start_index) {
            if !song.path.starts_with(dir) {
                return;
            }
            self.undo_songs.push(Arc::clone(song));
        }
    }
    /// Adds all songs from directory `dir` to `self.undo_songs`, so their
    /// info can be recovered using `LibraryRequest::UndoRemovedDirectory`
    fn undo_removed_directory(&mut self, dir: PathBuf) {
        self.cancel_library_build_blocking(Instant::now());
        self.missing_songs.extend(mem::take(&mut self.undo_songs));
        self.config.add_library(dir);
    }

    /// Starts a queue of all songs found within the specified `paths`,
    /// recursively. Does nothing if no song files were found.
    ///
    /// # Errors
    /// The function errors if either the player or UI channel receiver is closed
    pub fn play_from_paths<'i, I: Iterator<Item = &'i str>>(
        &mut self,
        paths: I,
    ) -> Result<(), Box<dyn Error>> {
        let queue = self.songs_from_paths(paths);
        if queue.is_empty() {
            return Ok(());
        }
        let player_tx = player_tx();
        player_tx.send(PlayerRequest::LoadQueue {
            queue,
            shuffled: None,
            track: 0,
        })?;
        player_tx.send(PlayerRequest::TogglePlay(Some(true)))?;
        let ui_tx = ui_tx();
        ui_tx.send_blocking(UpdateUI::OpenSheet(false))?;
        ui_tx.send_blocking(UpdateUI::FocusPlaying)?;
        Ok(())
    }

    /// Takes a list of file or directory paths and returns a queue
    #[inline]
    #[must_use]
    pub fn songs_from_paths<'i, I: Iterator<Item = &'i str>>(&self, paths: I) -> Vec<QueueItem> {
        let mut queue = Vec::with_capacity(paths.size_hint().0);
        for file in paths {
            if file_supported(file) {
                queue.push(QueueItem::Song(self.find_song_or_new(Path::new(file))));
            } else if file == "Pause" {
                queue.push(QueueItem::new_stopper(false));
            } else if file == "Close Player" {
                queue.push(QueueItem::new_stopper(true));
            } else {
                self.extend_queue_from_dir(&mut queue, file);
            }
        }
        queue
    }
    /// Attempts to locate the given `file` within the library and
    /// returns it, otherwise it returns a new `SharedSong`
    #[inline]
    #[must_use]
    fn find_song_or_new(&self, path: &Path) -> SharedSong {
        if (self.config.directories().iter()).any(|dir| path.starts_with(dir))
            && let Ok(index) = self.songs.find_song(path)
        {
            // SAFETY: `index` is `Ok`, therefore within bounds
            return Arc::clone(unsafe { self.songs.get_unchecked(index) });
        }
        SharedSong::from_path(path.to_path_buf())
    }
    /// Extends `queue` with songs found on disk within `dir`. If files are
    /// part of the music library, their existing instances will be used.
    ///
    /// The input `dir` must be a directory and exist on disk, otherwise
    /// the function does nothing.
    ///
    /// # Panics
    /// The function panics if any contained file paths are not valid UTF-8
    fn extend_queue_from_dir(&self, queue: &mut Vec<QueueItem>, dir: &str) {
        let path = PathBuf::from(&dir);
        if !path.is_dir() || !path.exists() {
            return;
        }
        let mut songs = Vec::with_capacity(16);
        let _ = visit_dirs(path, &mut |file_path| {
            if !file_path.extension().is_some_and(extension_supported) {
                return;
            }

            let song = self.find_song_or_new(&file_path);
            match songs.binary_search_by(|existing: &QueueItem| {
                // SAFETY: Only the `Song` variant is ever inserted into `songs`
                unsafe { &existing.as_song_unchecked().path }.cmp(&file_path)
            }) {
                Err(index) | Ok(index) => songs.insert(index, QueueItem::Song(song)),
            }
        });
        queue.extend(songs);
    }

    /// Serializes `songs` and writes the data to disk,
    /// so the library can be loaded faster next time
    ///
    /// File path is determined by `songs_file()`
    #[inline]
    fn serialize_songs(songs: &Songs) {
        let songs_data = (songs.iter())
            .map(|song| song.serialize() + "\n")
            .collect::<String>();
        match write_file_create_dir_all(
            songs_file(),
            [
                "\
/--------------------------------------------------------------------\\
|             Editing this file is usually not necessary             |
|                                                                    |
| Hint: Paths of moved and renamed files are corrected automatically |
|                                                                    |
|  Hint: You can append the contents of a different `songs` file to  |
|  this one to combine their data (such as ratings and play counts)  |
\\--------------------------------------------------------------------/
\n",
                &songs_data,
            ]
            .concat(),
        ) {
            Ok(()) => println!("Library song info has been successfully written to disk"),
            Err(e) => eprintln!("Problems writing the library state to disk: \n{e}"),
        }
    }

    /// Reads the serialized song info from disk and returns them,
    /// so they can be assigned directly to `self.songs`
    ///
    /// File path is determined by `songs_file()`
    #[inline]
    #[must_use]
    fn deserialize_songs() -> Songs {
        match fs::read_to_string(songs_file()) {
            Ok(data) => data
                .trim_end()
                .split("\n\n")
                .skip(1) // Skip the note written at the top of the `songs` file
                .filter_map(SharedSong::deserialize)
                .collect(),
            Err(_) => Vec::with_capacity(512), // Estimate to reduce reallocations
        }
    }

    /// Loops through all library songs, and adds their
    /// user-assigned tags to the global tag list
    ///
    /// Call this after deserializing songs
    #[inline]
    pub fn build_global_tag_list(&self) {
        let mut global_tags_writer = tag_list::write_global_tags();
        global_tags_writer.inner_mut().clear();
        for song in &self.songs {
            for tag in song.info().user().tags() {
                global_tags_writer.add(tag.to_owned());
            }
        }
    }

    /// Renames `tag` to `new_tag` on all songs in the library (including missing ones),
    /// and notifies when finished through `notify_done`
    ///
    /// # Panics
    /// The function panics if it encounters a poisoned `Mutex`
    pub fn rename_tag(&self, tag: &str, new_name: &str, notify_done: &mpsc::Sender<()>) {
        for song in self.songs.iter().chain(self.missing_songs.iter()) {
            if let Some(album) = &*song.get_album() {
                song.info()
                    .remove_tag_and(tag, &mut album.lock().unwrap(), |info, album| {
                        info.add_tag(new_name.to_owned(), album);
                    });
            }
        }
        notify_done.send(()).expect(EXP_RX);
    }

    /// Consumes `self`, writes the configuration to disk and shuts down gracefully
    ///
    /// # Panics
    /// The function panics if it encounters a poisoned `Mutex`
    fn shutdown(mut self) {
        STATE.store(STATE_CANCEL, atomic::Ordering::Release);
        (self.missing_songs).extend(mem::take(&mut *self.check_moved.lock().unwrap()));
        for missing in self.missing_songs {
            // Re-insert missing songs so their info is kept
            if let Err(index) = self.songs.find_song(&missing.path) {
                self.songs.insert(index, missing);
            }
        }
        Library::serialize_songs(&self.songs);
        self.tasks.shutdown();
    }
}

/// Returns `true` if the specified file has a supported extension,
/// or `false` if it does not
#[inline]
#[must_use]
pub fn file_supported(file: &str) -> bool {
    match file.rsplit_once('.') {
        Some((_, extension)) => extension_supported(&extension.to_lowercase()),
        None => false,
    }
}

/// Returns `true` if the specified extension is supported, or `false` if it is not
#[inline]
#[must_use]
pub fn extension_supported<S: PartialEq<str> + ?Sized>(extension: &S) -> bool {
    FILE_SUPPORT.iter().any(|&ext| extension == ext)
}
