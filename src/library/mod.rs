use core::sync::atomic::{self, AtomicBool};
use core::{cmp::Ordering, error::Error, mem};
use gio::prelude::FileExt;
use gtk::gio;
use rand::random_range;
use std::path::Path;
use std::sync::{Arc, Mutex, MutexGuard, OnceLock, mpsc};
use std::time::{Duration, Instant};
use std::{fs, thread};

pub mod album;
pub mod artist;
pub mod config;
pub mod song;

pub use album::{Album, SharedAlbum, SortedAlbumSongs};
pub use artist::{Artist, SharedArtist, SortedArtistAlbums};
pub use config::{FILE_SUPPORT, LibraryConfig};
pub use song::{SharedSong, SharedSongExt, Song, SongInfo, SongInfoLoader};

use crate::UI_TIMEOUT;
use crate::excuses::EXP_RX;
use crate::library::album::NewSharedAlbum;
use crate::library::artist::NewSharedArtist;
use crate::player::{PlayerRequest, QueueItem, player_tx};
use crate::ui::{UpdateUI, ui_tx};
use crate::util::tasks::{BoxedTask, Runner};
use crate::{songs_file, util::visit_dirs};

pub struct Library {
    songs: Songs,
    albums: Albums,
    artists: Artists,
    missing_songs: Songs,
    check_moved: Arc<Mutex<Songs>>,
    undo_songs: Songs,

    queue_initialized: bool,
    cancel_pending: Arc<AtomicBool>,

    on_build_succeeded: Vec<LibraryTask>,
    on_build_stopped: Vec<LibraryTask>,

    tasks: Runner,
    pub config: LibraryConfig,
    rx: mpsc::Receiver<LibraryRequest>,
}

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
    fn find_song(&self, uri: &str, trim_start: usize) -> Result<usize, usize>;
}
impl SortedSongs for Songs {
    #[inline]
    fn find_song(&self, uri: &str, trim_start: usize) -> Result<usize, usize> {
        let trimmed_uri = &uri[trim_start..];
        self.binary_search_by(|song| {
            assert!(song.uri.len() >= trim_start);
            // SAFETY: Assert above ensures `trim_start` is within bounds,
            // `song.uri` cannot contain large UTF-8 characters because it
            // is a URI, assigned from `gio::File::uri`
            unsafe { song.uri.get_unchecked(trim_start..) }.cmp(trimmed_uri)
        })
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
            match album.artist.lock().unwrap().name.cmp(&info.album_artist) {
                Ordering::Equal => album.title.cmp(&info.album),
                ordering => ordering,
            }
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
            let rand_index = random_range(0..shuffled.len());
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
}
impl SortedArtists for Artists {
    #[inline]
    fn find_artist(&self, info: &SongInfo) -> Result<usize, usize> {
        self.binary_search_by(|artist| artist.lock().unwrap().name.cmp(&info.album_artist))
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
            let rand_index = random_range(0..shuffled.len());
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

pub enum LibraryRequest {
    /// Rebuilds the library
    ///
    /// # Note:
    /// In most cases, `CancelRebuild` should be called first
    Rebuild,
    /// Cancels the current library build and pauses the thread pool requests
    /// until all current tasks finish running
    CancelRebuild,

    /// Starts a player queue using the given file or directory paths
    QueueFromPaths(Box<[String]>),

    /// Adds a new library directory to the configuration
    AddLibrary(Box<str>),
    /// Removes the library directory at the given index from the configuration
    RemoveLibrary(usize),

    /// Remembers the removed directory for undo
    RegisterUndoDirectory(String),
    /// Re-adds the removed directory and restores its library data
    UndoRemovedDirectory(String),

    /// Runs the given task on the thread pool, in the background
    RunTask(BoxedTask),
    /// Runs the given task on the library thread directly, with mutable access to the `Library`
    RunLibraryTask(LibraryTask),
    /// Runs the given task once the library build successfully completes in full
    OnBuildSucceeded(LibraryTask),
    /// Runs the given task once the library build is done, regardless if it succeeded or failed
    OnBuildStopped(LibraryTask),

    /// Cleanly shuts down the library and thread pool, and writes the configuration data to disk
    Uninit,
}

impl Library {
    /// Constructs a new instance of `Library`
    #[inline]
    #[must_use]
    pub fn init(config: LibraryConfig, library_rx: mpsc::Receiver<LibraryRequest>) -> Library {
        Library {
            songs: Vec::new(),
            albums: Vec::new(),
            artists: Vec::new(),
            missing_songs: Vec::new(),
            check_moved: Arc::new(Mutex::new(Vec::new())),
            undo_songs: Vec::new(),

            queue_initialized: false,
            cancel_pending: Arc::new(AtomicBool::new(false)),

            on_build_succeeded: Vec::new(),
            on_build_stopped: Vec::new(),

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
    /// a poisoned `Mutex` is passed
    #[inline]
    pub fn request_handler(mut self) -> Result<(), Box<dyn Error>> {
        loop {
            match self.rx.recv()? {
                LibraryRequest::RunTask(task) => self.tasks.run(task),

                LibraryRequest::QueueFromPaths(paths) => self.play_from_paths(
                    paths.iter().map(|path| &**path), //
                )?,

                LibraryRequest::CancelRebuild => self.cancel_library_build(),
                LibraryRequest::Rebuild => self.discover_files(),
                LibraryRequest::OnBuildSucceeded(f) => self.on_build_succeeded.push(f),
                LibraryRequest::OnBuildStopped(f) => self.on_build_stopped.push(f),
                LibraryRequest::RunLibraryTask(f) => f(&mut self),

                LibraryRequest::AddLibrary(dir) => self.config.add_library(dir.to_string()),
                LibraryRequest::RemoveLibrary(index) => self.config.remove_library(index),

                LibraryRequest::RegisterUndoDirectory(dir) => self.register_undo_directory(dir),
                LibraryRequest::UndoRemovedDirectory(dir) => self.undo_removed_directory(dir),

                #[allow(clippy::unit_arg)]
                LibraryRequest::Uninit => return Ok(self.shutdown()),
            }
        }
    }

    /// Locates song files within the configured directories, assigns
    /// them to `self.songs`, inserts any new files into `self.songs`,
    /// then runs `create_connections()` in a background process. If
    /// uninitialized, the data from disk is used first. Song entries
    /// already present in `self.songs` are preserved, and only new
    /// songs are added.
    ///
    /// # Panics
    /// The function panics if the library channel is closed
    pub fn discover_files(&mut self) {
        let mut songs = match self.songs.is_empty() {
            true => Library::deserialize_songs(),
            false => mem::take(&mut self.songs),
        };

        for library_path in self.config.directories() {
            let _ = visit_dirs(Path::new(&library_path), &mut |f| {
                let file = f.path();
                if !file.extension().is_some_and(extension_supported) {
                    return;
                }

                // Add the song to the library if it is new
                let file = gio::File::for_path(file);
                let uri = file.uri().to_string();
                if let Err(index) = songs.find_song(&uri, self.config.uri_opt()) {
                    songs.insert(index, SharedSong::from_file(file, uri));
                }
            })
            .inspect_err(|e| eprintln!("Error reading '{library_path}': {e}"));
        }
        self.songs.clone_from(&songs);

        self.tasks.run({
            let missing = mem::take(&mut self.missing_songs);
            let check = Arc::clone(&self.check_moved);
            let config = self.config.clone();
            let cancel = Arc::clone(&self.cancel_pending);
            let n_workers = self.tasks.num_workers();
            move || match Library::create_connections(
                songs, missing, &check, &config, &cancel, n_workers,
            ) {
                Ok(()) => (),
                Err(e) => eprintln!("`Library::create_connections`: {e}"),
            }
        });
    }

    /// Creates connections between library `songs`/`albums`/`artists`, and
    /// validates `songs` using `validate_songs()` and `merge_moved_entries()`
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
        check_moved: &Arc<Mutex<Songs>>,
        config: &LibraryConfig,
        cancel: &Arc<AtomicBool>,
        num_workers: usize,
    ) -> Result<(), Box<dyn Error>> {
        let num_tasks = num_workers - 2;
        let library_tx = library_tx();
        let ui_tx = ui_tx();

        Library::validate_songs(&mut songs, &mut missing, check_moved, config);

        #[cfg(feature = "startup-logs")]
        println!("Library songs validation complete");

        let file_times = Arc::new(Mutex::new(()));
        Library::run_task(library_tx, {
            let cancel = Arc::clone(cancel);
            let songs = songs.clone();
            let busy = Arc::clone(&file_times);
            move || {
                let _ = library_tx.send(LibraryRequest::RunLibraryTask(Box::new(move |library| {
                    library.set_missing_songs(missing);
                })));

                // Check file modification times in the background
                let file_times_guard = busy.lock().unwrap();
                let mut needs_rebuild = false;
                for song in &songs {
                    if cancel.load(atomic::Ordering::Relaxed) {
                        return;
                    }
                    let mut info = song.info();
                    let modification_time = info.file_modification_time(|info| {
                        eprintln!(
                            "WARNING: Could not determine modification time for '{:?}'; skipping...",
                            info.file_path()
                        );
                        info.known_modification_time()
                    });
                    if modification_time == info.known_modification_time()
                        || modification_time == -1
                    {
                        continue;
                    }
                    needs_rebuild |= info.inspect_basic_mut().take().is_some();
                    info.invalidate_thumbnail();
                }
                // If files were modified, queue another rebuild so the new info gets loaded
                if needs_rebuild && !cancel.swap(true, atomic::Ordering::Relaxed) {
                    let _ = library_tx.send(LibraryRequest::CancelRebuild);
                    let _ = library_tx.send(LibraryRequest::OnBuildStopped(Box::new(|_| {
                        ui_tx.send(UpdateUI::Progress(Some(0.0))).expect(EXP_RX);
                        println!("Rebuilding because files were modified");

                        // Sending rebuild request through the channel instead of running directly,
                        // so it waits for any cancellations to complete in full before rebuilding
                        library_tx.send(LibraryRequest::Rebuild).expect(EXP_RX);
                    })));
                    drop(file_times_guard);
                    println!("Modifications detected, library will rebuild shortly");
                    return;
                }

                drop(file_times_guard);
            }
        });

        if let Ok(check_moved) = check_moved.lock()
            && !check_moved.is_empty()
        {
            Library::merge_moved_entries(&songs, check_moved, config, cancel, num_tasks);
        }

        #[cfg(feature = "startup-logs")]
        println!("Merged moved file entries");

        Library::run_task(library_tx, {
            let cancel = Arc::clone(cancel);
            let songs = songs.clone();
            move || {
                // Wait before starting background tasks in case they aren't needed
                thread::sleep(Duration::from_millis(100));
                if cancel.load(atomic::Ordering::Relaxed) {
                    return;
                }

                // Prepare the songs to distribute between background tasks
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

                println!("Starting {num_tasks} background tasks to load the song info");
                for songs in worker_songs {
                    let cancel = Arc::clone(&cancel);
                    Library::run_task(library_tx, move || {
                        for song in songs {
                            if cancel.load(atomic::Ordering::Relaxed) {
                                #[cfg(debug_assertions)]
                                println!("Song info task was cancelled");
                                return;
                            }
                            drop(song.info().try_load_basic());
                        }
                    });
                }
            }
        });

        let mut albums = Vec::with_capacity(songs.len() / 16);
        let mut artists = Vec::with_capacity(songs.len() / 64);

        let progress_step = 1.0 / songs.len() as f64;
        let mut progress = 0.0;
        let mut timer = Instant::now();

        for song in &songs {
            let mut info = song.info();
            let song_info_lock = info.load_basic();
            let song_info = song_info_lock.as_ref().unwrap();

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
                        drop(song_info_lock);
                        drop(album_locked);
                        drop(info);

                        // Associate the song with its album
                        song.set_album(Arc::clone(album));
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
                        drop(song_info_lock);
                        drop(artist_locked);
                        drop(info);

                        // Add to the library albums as well
                        albums.insert(album_index, Arc::clone(&album));

                        // Associate the song with its album
                        song.set_album(album);
                    }
                },
                Err(artist_index) => {
                    // Create the artist and album connected pair
                    let (artist, album) = SharedArtist::new_artist_album_pair(
                        song_info, //
                        Arc::clone(song),
                    );
                    drop(song_info_lock);
                    drop(info);

                    // Add to the library albums as well
                    match album_index {
                        Err(index) | Ok(index) => albums.insert(index, Arc::clone(&album)),
                    }

                    // Add the artist entry
                    artists.insert(artist_index, artist);

                    // Associate the song with its album
                    song.set_album(album);
                }
            }

            progress += progress_step;
            if timer.elapsed() > UI_TIMEOUT {
                timer = Instant::now();
                if cancel.load(atomic::Ordering::Relaxed) {
                    let _ = ui_tx.send(UpdateUI::Progress(None));
                    break;
                }
                let _ = ui_tx.send(UpdateUI::Progress(Some(progress)));
            }
        }

        #[cfg(feature = "startup-logs")]
        println!("Library connections have finished building");

        library_tx.send(LibraryRequest::RunLibraryTask(Box::new({
            let cancel = cancel.clone();
            move |library| {
                library.set_artists(artists);
                library.set_albums(albums);
                library.set_songs(songs);

                ui_tx.send(UpdateUI::Progress(None)).expect(EXP_RX);

                // Wait for the file modification times to be fully checked
                drop(file_times.lock().unwrap());

                library.build_stopped();

                if !cancel.load(atomic::Ordering::Relaxed) {
                    // Cancel any background tasks which might still be running
                    library.cancel_library_build();
                    library.build_succeeded();
                }
            }
        })))?;

        match cancel.load(atomic::Ordering::Relaxed) {
            false => Ok(()),
            true => Err("Cancelled")?,
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
        unchecked: &Arc<Mutex<Songs>>,
        config: &LibraryConfig,
    ) {
        let old_songs = [
            mem::replace(songs, Vec::with_capacity(songs.len())),
            mem::take(&mut *unchecked.lock().unwrap()),
            mem::take(missing),
        ]
        .concat();
        let mut possibly_moved = Vec::new();
        let mut libraries = Vec::with_capacity(config.directories().len());
        let mut missing_libraries = Vec::new();
        for (index, dir) in config.directory_uris().iter().enumerate() {
            let opt_dir = &dir[config.uri_opt()..];
            match fs::exists(&config.directories()[index]) {
                Ok(true) => libraries.push(opt_dir),
                _ => missing_libraries.push(opt_dir),
            }
        }
        'iter: for song in old_songs {
            let opt_uri = &song.uri[config.uri_opt()..];
            match songs.find_song(&song.uri, config.uri_opt()) {
                // Valid song entry
                Err(index)
                    if (song.file.path())
                        .is_some_and(|path| fs::exists(path).unwrap_or_default()) =>
                {
                    // Filter songs outside of `config.directories`
                    if libraries.iter().any(|dir| opt_uri.starts_with(dir)) {
                        songs.insert(index, song);
                        continue 'iter;
                    }
                    // IDEA: To disable libraries, move `songs` into `disabled_songs`

                    // The file may have been copied to an active library
                    possibly_moved.push(song);
                }
                // Missing file
                Err(_) => {
                    match missing.find_song(&song.uri, config.uri_opt()) {
                        // New missing song entry
                        Err(index) => {
                            // Only remember missing files if they are within
                            // a library directory which is currently missing
                            // (otherwise, they were either moved or removed)
                            if (missing_libraries.iter()).any(|dir| opt_uri.starts_with(dir)) {
                                // #[cfg(debug_assertions)]
                                // println!(
                                //     "Remembering {} because its library is missing",
                                //     info.filename()
                                // );
                                missing.insert(index, song);
                                continue 'iter;
                            }

                            possibly_moved.push(song);
                        }
                        // Duplicate missing song entry
                        Ok(index) => {
                            song.info().user().merge_with(&missing[index].info().user());
                            drop(song);
                        }
                    }
                }
                // Duplicate entry
                Ok(index) => {
                    #[cfg(debug_assertions)]
                    println!("Resolving duplicate entry: {}", song.uri);
                    // SAFETY: `index` is `Ok`, therefore within bounds
                    (unsafe { songs.get_unchecked(index) }.info().user())
                        .merge_with(&song.info().user());
                    drop(song);
                }
            }
        }

        mem::swap(&mut *unchecked.lock().unwrap(), &mut possibly_moved);
    }

    /// Attempts to locate missing files if they were moved and merges
    /// them with the existing song entries so their info is preserved
    ///
    /// # Panics
    /// The function panics if the UI channel receiver is unititialized
    /// or closed, or if the `check_moved` mutex is in a poisoned state
    fn merge_moved_entries(
        songs: &Songs,
        mut check_moved: MutexGuard<'_, Songs>,
        config: &LibraryConfig,
        cancel: &Arc<AtomicBool>,
        num_tasks: usize,
    ) {
        #[inline]
        #[must_use]
        fn merge_if_matching(info: &mut SongInfoLoader, cmp_info: &SongInfoLoader) -> bool {
            drop(info.load_basic()); // Load info for more accurate matching
            if cmp_info.matches(info) {
                // Copy the user-assigned song info to the new entry
                #[cfg(debug_assertions)]
                println!(
                    "Found moved file:\n{} -> {}",
                    cmp_info.file_path(),
                    info.file_path()
                );
                info.user().merge_with(&cmp_info.user());
                let _ = fs::rename(cmp_info.thumbnail_file_path(), info.thumbnail_file_path());
                return true;
            }
            let _ = fs::remove_file(cmp_info.thumbnail_file_path());
            false
        }

        let (missing_tx, missing_rx) = mpsc::sync_channel::<Option<(usize, Arc<Song>)>>(0);
        let missing_rx = Arc::new(Mutex::new(missing_rx));
        let uri_opt = Arc::new(config.uri_opt());
        let songs = Arc::new(songs.clone());
        let moved_count = check_moved.len() as f64;
        let cancelled = Arc::new(Mutex::new(Vec::new()));
        let library_tx = library_tx();

        for _ in 0..num_tasks {
            let songs = Arc::clone(&songs);
            let uri_opt = Arc::clone(&uri_opt);
            let missing_rx = Arc::clone(&missing_rx);
            let cancelled = Arc::clone(&cancelled);
            let cancel = Arc::clone(cancel);

            Library::run_task(library_tx, move || {
                let mut timer = Instant::now();
                let cancellation_interval = Duration::from_millis(100);
                while let Some((i, missing)) = missing_rx.lock().unwrap().recv().unwrap() {
                    // Optimization: start with an initial guess and expand outwards
                    let mut guess = match songs.find_song(&missing.uri, *uri_opt) {
                        Err(index) | Ok(index) => index,
                    };
                    if guess == 0 || guess >= songs.len() {
                        guess = (1.max(i) as f64 / moved_count * songs.len() as f64) as usize;
                    }

                    let old_info = missing.info();
                    let (mut left, mut right) = (songs[..guess].iter(), songs[guess..].iter());
                    while match (left.next_back(), right.next()) {
                        (_, Some(song)) if merge_if_matching(&mut song.info(), &old_info) => false,
                        (Some(song), _) if merge_if_matching(&mut song.info(), &old_info) => false,
                        (None, None) => false,
                        _ => true, // Loop until either the song is found or all songs were checked
                    } {
                        if timer.elapsed() > cancellation_interval {
                            timer = Instant::now();
                            if cancel.load(atomic::Ordering::Relaxed) {
                                cancelled.lock().unwrap().push(missing);
                                return;
                            }
                        }
                    }
                }
            });
        }
        drop(missing_rx);

        let ui_tx = ui_tx();
        let progress_step = 1.0 / check_moved.len() as f64;
        let mut progress = 0.0;
        let mut timer = Instant::now();

        // Show the progress bar and block until done
        while missing_tx
            .send(check_moved.pop().map(|song| (check_moved.len(), song)))
            .is_ok()
        {
            progress += progress_step;
            if timer.elapsed() > UI_TIMEOUT {
                timer = Instant::now();
                if cancel.load(atomic::Ordering::Relaxed) {
                    while missing_tx.send(None).is_ok() {
                        // Sending `None` until all workers stop
                    }
                    check_moved.extend(mem::take(&mut *cancelled.lock().unwrap()));
                    drop(check_moved);
                    break;
                }
                let _ = ui_tx.send(UpdateUI::Progress(Some(progress)));
            }
        }
    }

    /// Cancels any currently running library build operation
    pub fn cancel_library_build(&self) {
        self.cancel_pending.store(true, atomic::Ordering::Relaxed);
        self.tasks.await_all_tasks();
        let cancel_pending = Arc::clone(&self.cancel_pending);
        self.tasks.run(move || {
            cancel_pending.store(false, atomic::Ordering::Relaxed);
        });
    }

    /// Cancels any currently running library build operation
    /// and blocks the current thread until fully cancelled
    pub fn cancel_library_build_blocking(&self) {
        self.cancel_pending.store(true, atomic::Ordering::Relaxed);
        self.tasks.await_all_tasks();
        let cancel_pending = Arc::clone(&self.cancel_pending);
        let library_thread = thread::current();
        self.tasks.run(move || {
            cancel_pending.store(false, atomic::Ordering::Relaxed);
            library_thread.unpark();
        });
        // Parking the thread in a loop until cancellation, because
        // threads can supposedly unpark themselves in some cases
        while self.cancel_pending.load(atomic::Ordering::Relaxed) {
            thread::park();
        }
    }

    /// Returns an `Option<SharedSong>` depending on whether the song
    /// was found within the library or not
    ///
    /// # Panics
    /// Panics if the found item's associated album or ortist `Mutex`
    /// is in a poisoned state
    #[inline]
    #[must_use]
    pub fn locate_song_by_info(&self, info: &SongInfo) -> Option<SharedSong> {
        if info.title.is_empty() {
            return None;
        }

        let artist = match self.artists.find_artist(info) {
            // SAFETY: `Ok` variant returned by `find_artist` is always within bounds
            Ok(artist_index) => unsafe { self.artists.get_unchecked(artist_index).lock().unwrap() },
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

    /// Replaces `self.songs` with `songs`
    ///
    /// # Panics
    /// The function panics if the UI channel receiver is closed
    #[inline]
    fn set_songs(&mut self, songs: Songs) {
        (ui_tx().send(UpdateUI::SetLibrarySongs(songs.clone()))).expect(EXP_RX);
        self.songs = songs;
    }
    /// Replaces `self.albums` with `albums`
    ///
    /// # Panics
    /// The function panics if the UI channel receiver is closed
    #[inline]
    fn set_albums(&mut self, albums: Albums) {
        (ui_tx().send(UpdateUI::SetLibraryAlbums(albums.clone()))).expect(EXP_RX);
        self.albums = albums;
    }
    /// Replaces `self.artists` with `artists`
    ///
    /// # Panics
    /// The function panics if the UI channel receiver is closed
    #[inline]
    fn set_artists(&mut self, artists: Artists) {
        (ui_tx().send(UpdateUI::SetLibraryArtists(artists.clone()))).expect(EXP_RX);
        self.artists = artists;
    }
    /// Replaces `self.missing_songs` with `missing_songs`
    #[inline]
    fn set_missing_songs(&mut self, missing_songs: Songs) {
        self.missing_songs = missing_songs;
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

    /// Adds all songs from directory `dir` to `self.undo_songs`, so their
    /// info can be recovered using `LibraryRequest::UndoRemovedDirectory`
    fn register_undo_directory(&mut self, dir: String) {
        let dir_uri = &*gio::File::for_path(dir).uri();
        let Err(start_index) =
            (self.songs).find_song(dir_uri, self.config.uri_opt().min(dir_uri.len()))
        else {
            unreachable!( /* `dir_uri` is a directory, not a song file */ )
        };
        for song in self.songs.iter().skip(start_index) {
            if !song.uri.starts_with(dir_uri) {
                return;
            }
            self.undo_songs.push(Arc::clone(song));
        }
    }
    /// Adds all songs from directory `dir` to `self.undo_songs`, so their
    /// info can be recovered using `LibraryRequest::UndoRemovedDirectory`
    fn undo_removed_directory(&mut self, dir: String) {
        self.cancel_library_build_blocking();
        self.missing_songs.extend(mem::take(&mut self.undo_songs));
        self.config.add_library(dir);
    }

    /// Starts a queue of all songs in the library
    ///
    /// # Errors
    /// The function errors if either the player or UI channel receiver is closed
    pub fn play_all_songs(&self, shuffle: bool) -> Result<(), Box<dyn Error>> {
        let player_tx = player_tx();
        player_tx.send(PlayerRequest::LoadQueue(
            self.songs.to_queue(),
            match shuffle {
                true => Some(vec![]),
                false => None,
            },
            0,
        ))?;
        player_tx.send(PlayerRequest::TogglePlay(Some(true)))?;
        let ui_tx = ui_tx();
        ui_tx.send(UpdateUI::OpenSheet(false))?;
        ui_tx.send(UpdateUI::FocusPlaying)?;
        Ok(())
    }

    /// Starts a queue of all albums in the library
    ///
    /// # Errors
    /// The function errors if either the player or UI channel receiver is closed
    pub fn play_all_albums(&self) -> Result<(), Box<dyn Error>> {
        let player_tx = player_tx();
        player_tx.send(PlayerRequest::LoadQueue(self.albums.to_queue(), None, 0))?;
        player_tx.send(PlayerRequest::TogglePlay(Some(true)))?;
        let ui_tx = ui_tx();
        ui_tx.send(UpdateUI::OpenSheet(false))?;
        ui_tx.send(UpdateUI::FocusPlaying)?;
        Ok(())
    }

    /// Starts a randomly ordered queue of all albums in the library
    ///
    /// # Errors
    /// The function errors if either the player or UI channel receiver is closed
    pub fn shuffle_all_albums(&self) -> Result<(), Box<dyn Error>> {
        let player_tx = player_tx();
        player_tx.send(PlayerRequest::LoadQueue(
            self.albums.to_shuffled_queue(),
            None,
            0,
        ))?;
        let ui_tx = ui_tx();
        player_tx.send(PlayerRequest::TogglePlay(Some(true)))?;
        ui_tx.send(UpdateUI::OpenSheet(false))?;
        ui_tx.send(UpdateUI::FocusPlaying)?;
        Ok(())
    }

    /// Starts a queue of all albums in the library
    ///
    /// # Errors
    /// The function errors if either the player or UI channel receiver is closed
    pub fn play_all_artists(&self) -> Result<(), Box<dyn Error>> {
        let player_tx = player_tx();
        player_tx.send(PlayerRequest::LoadQueue(self.artists.to_queue(), None, 0))?;
        player_tx.send(PlayerRequest::TogglePlay(Some(true)))?;
        let ui_tx = ui_tx();
        ui_tx.send(UpdateUI::OpenSheet(false))?;
        ui_tx.send(UpdateUI::FocusPlaying)?;
        Ok(())
    }

    /// Starts a randomly ordered queue of all artists in the library
    ///
    /// # Errors
    /// The function errors if either the player or UI channel receiver is closed
    pub fn shuffle_all_artists(&self) -> Result<(), Box<dyn Error>> {
        let player_tx = player_tx();
        player_tx.send(PlayerRequest::LoadQueue(
            self.artists.to_shuffled_queue(),
            None,
            0,
        ))?;
        player_tx.send(PlayerRequest::TogglePlay(Some(true)))?;
        let ui_tx = ui_tx();
        ui_tx.send(UpdateUI::OpenSheet(false))?;
        ui_tx.send(UpdateUI::FocusPlaying)?;
        Ok(())
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
        player_tx.send(PlayerRequest::LoadQueue(queue, None, 0))?;
        player_tx.send(PlayerRequest::TogglePlay(Some(true)))?;
        let ui_tx = ui_tx();
        ui_tx.send(UpdateUI::OpenSheet(false))?;
        ui_tx.send(UpdateUI::FocusPlaying)?;
        self.queue_initialized = true;
        Ok(())
    }

    /// Takes a list of file or directory paths and returns a queue
    #[inline]
    #[must_use]
    pub fn songs_from_paths<'i, I: Iterator<Item = &'i str>>(&self, paths: I) -> Vec<QueueItem> {
        let mut queue = Vec::with_capacity(paths.size_hint().0);
        for file in paths {
            if file_supported(file) {
                queue.push(QueueItem::Song(
                    self.song_from_library_or_new(gio::File::for_path(file)),
                ));
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
    fn song_from_library_or_new(&self, file: gio::File) -> SharedSong {
        let file_uri = &*file.uri();
        // SAFETY: `file.uri()` converts special characters to regular ones
        if unsafe { self.config.uri_within_library(file_uri) }
            && let Ok(index) = self.songs.find_song(file_uri, self.config.uri_opt())
        {
            // SAFETY: `index` is `Ok`, therefore within bounds
            return Arc::clone(unsafe { self.songs.get_unchecked(index) });
        }
        SharedSong::from_file(file, file_uri.to_owned())
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
        let path = Path::new(&dir);
        if !path.is_dir() || !path.exists() {
            return;
        }
        let mut songs = Vec::with_capacity(16);
        let _ = visit_dirs(path, &mut |file| {
            let file_path = file.path();
            if !file_path.extension().is_some_and(extension_supported) {
                return;
            }

            let file = gio::File::for_path(&file_path);
            let song = self.song_from_library_or_new(file);
            match songs.binary_search_by(|existing: &QueueItem| {
                // SAFETY: Only the `Song` variant is ever inserted into `songs`
                unsafe { existing.as_song_unchecked().file.path().unwrap() }.cmp(&file_path)
            }) {
                Err(index) | Ok(index) => songs.insert(index, QueueItem::Song(song)),
            }
        });
        queue.extend(songs);
    }

    /// Serializes `songs` and writes the data to disk,
    /// so the library can be loaded faster next time
    ///
    /// Writes to a file called `songs` in `self.config.dir`
    #[inline]
    fn serialize_songs(songs: &Songs) {
        let serialized = (songs.iter())
            .map(|song| song.serlialize() + "\n")
            .collect::<String>();
        match fs::write(songs_file(), serialized.trim()) {
            Ok(()) => println!("Library song info has been successfully written to disk"),
            Err(e) => eprintln!("Problems writing the library state to disk: \n{e}"),
        }
    }

    /// Reads the serialized song info from disk and returns them,
    /// so they can be assigned directly to `self.songs`
    ///
    /// Reads from a file called `songs` in `self.config.dir`
    #[inline]
    #[must_use]
    fn deserialize_songs() -> Songs {
        match fs::read_to_string(songs_file()) {
            Ok(data) => (data.split("\n\n").filter_map(SharedSong::deserialize)).collect(),
            Err(_) => Vec::with_capacity(512), // Estimate to reduce reallocations
        }
    }

    /// Consumes `self`, writes the configuration to disk and shuts down gracefully
    ///
    /// # Panics
    /// The function panics if it encounters a poisoned `Mutex`
    fn shutdown(mut self) {
        self.cancel_pending.store(true, atomic::Ordering::Relaxed);
        (self.missing_songs).extend(mem::take(&mut *self.check_moved.lock().unwrap()));
        for missing in self.missing_songs {
            // Re-insert missing songs so their info is kept
            if let Err(index) = self.songs.find_song(&missing.uri, self.config.uri_opt()) {
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
