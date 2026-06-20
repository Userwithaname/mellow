use core::error::Error;
use gst::prelude::*;
use gst::{ClockTime, SeekFlags, State};
use std::sync::{OnceLock, mpsc};

use crate::UI_TIMEOUT;
use crate::excuses::{EXP_RX, INIT_ERR};
use crate::ui::{UpdateUI, ui_tx};
use crate::util::hint::{cold, unlikely};
use crate::util::wrap_index;

pub mod queue_item;
pub mod song_queue;

pub use queue_item::{QueueItem, SharedStopper};
pub use song_queue::{SongQueue, UndoAction};

static PLAYER_TX: OnceLock<mpsc::Sender<PlayerRequest>> = OnceLock::new();
/// Returns the channel sender for sending requests to the player using `PlayerRequest`
///
/// # Safety
/// Causes undefined behavior if called before `init_channels`
#[inline]
pub fn player_tx() -> &'static mpsc::Sender<PlayerRequest> {
    // SAFETY: `init_channels` runs in `Application::run`, before starting any threads
    unsafe { PLAYER_TX.get().unwrap_unchecked() }
}
/// Initializes the player channel sender accessed through `player_tx()`
///
/// # Errors
/// The function returns an error if `PLAYER_TX` has already been initialized
#[inline]
pub fn init_player_tx(
    player_tx: mpsc::Sender<PlayerRequest>,
) -> Result<(), mpsc::Sender<PlayerRequest>> {
    PLAYER_TX.set(player_tx)
}

pub enum PlayerRequest {
    /// Play or pause depending on the current state
    TogglePlay(Option<bool>),
    /// Skip to beginning or previous song
    SkipPrevious,
    /// Skip to the next song in the queue
    SkipNext,
    /// Skip to the specified index in the queue
    SkipTo(usize),
    /// Seek to a particular point in the song using a 0 to 1 value
    /// Using this will pause the player; send `SeekDone` to resume
    Seek(f64),
    /// Stop seeking and resume the player state
    SeekDone,
    /// Seek to a particular point in the song (for interactive seeking, use `Seek` instead)
    SeekToTime(ClockTime),
    /// Load the next song without clearing the stream
    LoadNext,
    /// Signaled from `GStreamer` to load next track before EOS (for gapless playback)
    SongEnd,

    /// Loads a new queue and an optional `shuffled` queue. Shuffle mode is set according to
    /// whether `shuffled` is `Some` or `None`. If `Some` but empty, a new shuffled queue is
    /// created.
    LoadQueue {
        queue: Vec<QueueItem>,
        shuffled: Option<Vec<usize>>,
        track: usize,
    },
    /// Appends multiple items to the current queue
    AppendQueue(Vec<QueueItem>),
    /// Appends an item to the current queue
    Append(QueueItem),
    /// Moves a queue item from index at `from` to `to`
    Reorder { from: usize, to: usize },
    /// Moves a queue item from index at `from` to `from + by`
    /// (the resulting position may wrap around the queue in repeat mode)
    Shift { from: usize, by: isize },
    /// Inserts an item into the queue
    InsertAt(Box<(usize, QueueItem)>),
    /// Inserts multiple items into the queue at once
    /// For proper behavior, indexes must be ordered from lowest to highest
    InsertItems(Vec<(usize, QueueItem)>),
    /// Inserts an item into the queue relative to the currently playing index
    InsertRelative(Box<(isize, QueueItem)>),
    /// Replaces each item at the index with a different `QueueItem`
    /// For example, this can be useful for correcting file paths of queue items
    ReplaceItems(Vec<(usize, QueueItem)>),
    /// Removes the item at the specified index from the queue
    RemoveItem(usize),
    /// Removes multiple items at the same time using the list of indices
    /// For proper behavior, indexes must be ordered from highest to lowest
    RemoveItems(Vec<usize>),
    /// Restores the queue to the previous snapshot
    Undo,

    /// Set the playback volume using a 0 to 1 value
    SetVolume(f64),
    /// Turn the shuffle mode on or off
    SetShuffle(bool),
    /// Turn the repeat mode on or off
    SetRepeat(bool),
    /// Turn gapless playback on or off
    SetGapless(bool),

    /// Attempts to correct file paths of moved song files in the queue
    ValidateFilePaths,

    /// Uninitializes the player and saves the current queue to disk
    /// (save queue: `bool`, save time: `bool`)
    Uninit(bool, bool),
}

// Manual `Debug` implementation is required by certain variants
impl core::fmt::Debug for PlayerRequest {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(
            f,
            "{}",
            match self {
                Self::TogglePlay(play) => format!("TogglePlay({play:?})"),
                Self::SkipPrevious => "SkipPrevious".to_owned(),
                Self::SkipNext => "SkipNext".to_owned(),
                Self::SkipTo(index) => format!("SkipTo({index})"),
                Self::Seek(pos) => format!("Seek({pos})"),
                Self::SeekDone => "SeekDone".to_owned(),
                Self::SeekToTime(time) => format!("SeekToTime({time})"),
                Self::LoadNext => "LoadNext".to_owned(),
                Self::SongEnd => "SongEnd".to_owned(),
                Self::LoadQueue {
                    queue,
                    shuffled: Some(shuffled),
                    track,
                } => format!(
                    "LoadQueue((…, Some(), {track})): {} items, {} shuffled",
                    queue.len(),
                    shuffled.len()
                ),
                Self::LoadQueue { queue, track, .. } =>
                    format!("LoadQueue((…, None, {track})): {} items", queue.len()),
                Self::AppendQueue(queue) => format!("AppendQueue(…): {} items", queue.len()),
                Self::Append(_) => "Append(…)".to_owned(),
                Self::Reorder { from, to } => format!("Reorder({from}, {to})"),
                Self::Shift { from, by } => format!("Shift({from}, {by})"),
                Self::InsertAt(item) => format!("InsertAt({}, …)", item.0),
                Self::InsertItems(items) => format!("InsertItems(…): {} items", items.len()),
                Self::InsertRelative(item) => format!("InsertRelative({}, …)", item.0),
                Self::ReplaceItems(items) => format!("ReplaceItems(…): {} items", items.len()),
                Self::RemoveItem(index) => format!("RemoveItem({index})"),
                Self::RemoveItems(indexes) => format!("RemoveItems{indexes:?}"),
                Self::Undo => String::from("Undo"),
                Self::SetVolume(volume) => format!("SetVolume({volume})"),
                Self::SetShuffle(shuffle) => format!("SetShuffle({shuffle})"),
                Self::SetRepeat(repeat) => format!("SetRepeat({repeat})"),
                Self::SetGapless(gapless) => format!("SetGapless({gapless})"),
                Self::ValidateFilePaths => "ValidateFilePaths".to_owned(),
                Self::Uninit(save, time) => format!("Uninit({save}, {time})"),
            }
        )
    }
}

pub struct Player {
    current_state: State,
    pending_state: Option<State>,
    /// Note: `self.queue.index()` returns the next track index instead of current when `true`
    next_song_loaded: bool,
    seeking: bool,

    gapless: bool,

    backend: gst::Element,
    bus: gst::Bus,
    rx: mpsc::Receiver<PlayerRequest>,

    pub queue: SongQueue,
}

// NOTE: Set `GST_DEBUG=3` to debug GStreamer
// https://gstreamer.freedesktop.org/documentation/tutorials/basic/debugging-tools.html

impl Player {
    /// Constructs a new instance of `Player`
    ///
    /// # Panics
    /// The function panics if initialization fails;
    /// initializing multiple times is not allowed
    #[inline]
    #[must_use]
    pub fn init(player_rx: mpsc::Receiver<PlayerRequest>) -> Player {
        gst::init().expect("Could not initialize GStreamer");

        let backend = gst::ElementFactory::make("playbin3")
            .build()
            .expect("Failed to create GStreamer element playbin3");
        let bus = backend.bus().expect(INIT_ERR);

        Player {
            queue: SongQueue::init(),

            gapless: true,

            current_state: State::Null,
            pending_state: None,
            next_song_loaded: false,
            seeking: false,

            backend,
            bus,
            rx: player_rx,
        }
    }

    /// Main controller loop which handles player requests
    ///
    /// # Errors
    /// The function may error upon handling a request:
    /// - If a required channel receiver is closed
    /// - Due to an unhandled `GStreamer` error
    ///
    /// # Panics
    /// The function may panic when handling a request
    /// in some cases, such as:
    /// - A required channel receiver is closed
    /// - A crash occurs in `GStreamer`
    pub fn controller(mut self) -> Result<(), Box<dyn Error>> {
        // Required for gapless playback
        self.backend.connect("about-to-finish", false, move |_| {
            // Cannot fail because the receiver is owned by `self`
            let _ = player_tx().send(PlayerRequest::SongEnd);
            None
        });

        loop {
            self.handle_gst_events();
            let Ok(player_request) = self.rx.recv_timeout(UI_TIMEOUT) else {
                self.ui_set_time();
                continue;
            };

            // Unit comparisons are used to avoid calling `continue`, so the branches fit on one line.
            // To update the player, use `== ()` or `true`
            // To skip updating the player, use `!= ()`, `false`, or `continue`
            #[allow(clippy::unit_cmp)]
            if match dbg!(player_request) {
                PlayerRequest::LoadQueue {
                    queue,
                    shuffled,
                    track: playing_index,
                } => self.load_queue(queue, shuffled, playing_index) == (),

                PlayerRequest::SeekToTime(time) => self.seek_to_time(time)? != (),
                PlayerRequest::Seek(pos) => self.seek_to_position_paused(pos)? != (),
                PlayerRequest::SeekDone => self.seek_done() == (),

                PlayerRequest::TogglePlay(None) => self.play_or_pause() == (),
                PlayerRequest::TogglePlay(Some(play)) => match self.current_state {
                    State::Playing if !play => self.play_or_pause() == (),
                    State::Playing => continue,
                    _ if play => self.play_or_pause() == (),
                    _ => continue,
                },

                PlayerRequest::SetVolume(vol) => self.set_volume(vol) != (),
                PlayerRequest::SetShuffle(shuffle) => self.queue.set_shuffle(shuffle) != (),
                PlayerRequest::SetRepeat(repeat) => self.queue.set_repeat(repeat) != (),
                PlayerRequest::SetGapless(gapless) => (self.gapless = gapless) != (),

                PlayerRequest::SkipPrevious => self.skip_prev_or_repeat()? == (),
                PlayerRequest::SkipNext => self.skip_next()? == (),
                PlayerRequest::SkipTo(index) => self.skip_to(index) == (),
                PlayerRequest::LoadNext if self.seeking => continue,
                PlayerRequest::SongEnd if let State::Null = self.current_state => continue,
                PlayerRequest::SongEnd if !self.can_use_gapless() => match self.seeking {
                    true => println!("Ignoring SongEnd while seeking") != (),
                    false => self.request_state(self.current_state) == (),
                },
                PlayerRequest::LoadNext | PlayerRequest::SongEnd => self.move_next(true) == (),

                PlayerRequest::AppendQueue(queue) => self.queue.append(&queue) != (),
                PlayerRequest::Append(item) => {
                    self.insert_to_queue(self.queue.len(), item);
                    self.queue.ui_update_queue();
                    true
                }
                PlayerRequest::Reorder { from, to } => self.reorder(from, to) == (),
                PlayerRequest::Shift { from, by } => self.shift(from, by) == (),
                PlayerRequest::InsertAt(item) => {
                    self.insert_to_queue(item.0, item.1);
                    self.queue.ui_update_queue();
                    self.queue.ui_validate_queue_subpage_index();
                    true
                }
                PlayerRequest::InsertItems(items) => {
                    for item in items {
                        self.insert_to_queue(item.0, item.1);
                    }
                    self.queue.ui_update_queue();
                    self.queue.ui_validate_queue_subpage_index();
                    true
                }
                PlayerRequest::InsertRelative(item) => {
                    self.insert_to_queue(
                        match item.0 >= 0 {
                            true => self.queue.index() + item.0 as usize,
                            false => self.queue.index() - -item.0 as usize,
                        },
                        item.1,
                    );
                    self.queue.ui_update_queue();
                    self.queue.ui_validate_queue_subpage_index();
                    true
                }
                PlayerRequest::ReplaceItems(items) => {
                    for (index, item) in items {
                        self.queue.replace(index, item);
                        if index == self.queue.index() {
                            self.queue.pending_track = true;
                        }
                    }
                    self.queue.ui_update_queue();
                    self.ui_update_song_info();
                    true
                }
                PlayerRequest::RemoveItem(index) => {
                    self.queue.create_snapshot_for_action(
                        UndoAction::Removed(vec![index]), //
                    );
                    self.remove_item(index);
                    self.queue.ui_update_queue();
                    self.queue.ui_validate_queue_subpage_index();
                    continue;
                }
                PlayerRequest::RemoveItems(indexes) => {
                    self.queue.create_snapshot_for_action(
                        UndoAction::Removed(indexes.clone()), //
                    );
                    for index in indexes {
                        self.remove_item(index);
                    }
                    self.queue.ui_update_queue();
                    continue;
                }
                PlayerRequest::Undo => self.queue.pefrofm_undo() == (),

                PlayerRequest::ValidateFilePaths => self.queue.validate_file_paths() == (),

                #[allow(clippy::unit_arg)]
                PlayerRequest::Uninit(save_queue, save_time) => {
                    return Ok(self.shutdown(save_queue, save_time));
                }
            } {
                self.update();
                self.ui_set_state();
            }
        }
    }

    /// Manages the playback state
    #[inline]
    fn update(&mut self) {
        if self.queue.is_empty() {
            #[cfg(debug_assertions)]
            println!("The queue is empty - player state update skipped");
            return;
        }

        let pending_track = self.queue.pending_track;

        let file_uri = match self.queue.current() {
            QueueItem::Song(song) => song.get_uri(),
            QueueItem::Stopper(_) => {
                if (self.queue.remove_current().as_stopper()).should_close_player() {
                    let _ = ui_tx().send(UpdateUI::RunAction("app.quit"));
                }
                self.queue.ui_update_queue();
                let _ = self.backend.set_state(State::Null);
                self.request_state(State::Paused);
                return self.update();
            }
        };

        if pending_track {
            println!("\n{file_uri}");
            self.backend.set_property("uri", file_uri);
            self.queue.pending_track = false;

            if self.current_state == State::Null {
                self.ui_update_song_info();
                self.queue.ui_update_queue_index();
                self.request_state(State::Paused);
                self.next_song_loaded = false;
            } else {
                self.next_song_loaded = true;
            }
        }

        if let Some(state) = self.pending_state.take() {
            match self.backend.set_state(state) {
                Ok(_) => self.current_state = state,
                Err(_) => self.force_stop_playback(),
            }
        }

        // Re-enable gapless playback (for example after track skip)
        self.backend.set_property("instant-uri", false);
    }

    /// Replaces the song queue with `queue` and skips to `index`.
    /// If `shuffled` is `Some`, shuffle mode is enabled, and the
    /// provided queue is used. If the `shuffled` queue is empty,
    /// a new one is created.
    ///
    /// # Panics
    /// Panics if the UI channel is closed
    #[inline]
    fn load_queue(&mut self, queue: Vec<QueueItem>, shuffled: Option<Vec<usize>>, index: usize) {
        if unlikely(index >= queue.len()) {
            if !queue.is_empty() {
                // Fallback to 0 if `index` is out of bounds
                return self.load_queue(queue, shuffled, 0);
            }

            let _ = self.backend.set_state(State::Null);
            self.queue.load_new(queue, shuffled);
            self.queue.ui_update_queue();
            self.ui_update_song_info();
            Self::ui_open_playing();
            return;
        }

        // Display the current song info in the UI as soon as possible
        let queue_item = QueueItem::clone(match &shuffled {
            None => &queue[index],
            Some(shuffled) => &queue[shuffled[index]],
        });
        (ui_tx().send(UpdateUI::SongInfo {
            item: QueueItem::clone(&queue_item),
            pause_after: queue.get(index + 1).is_some_and(QueueItem::is_stopper),
        }))
        .expect(EXP_RX);

        self.queue.load_new(queue, shuffled);
        self.skip_to(index);

        // Updating manually before using this thread to load the thumbnail
        self.update();

        // Ensure the current thumbnail is loaded before updating the UI queue
        queue_item.map_song(|song| drop(song.info().load_thumbnail()));
        let _ = ui_tx().send(UpdateUI::ExitQueueSelection);
        self.queue.ui_update_queue();
    }

    /// Starts or pauses playback depending on state
    fn play_or_pause(&mut self) {
        // WORKAROUND: Set `self.seeking = false` because the seek bar release callback
        // doesn't work on some systems. This makes the player function properly again
        // after pressing the pause button to resume the playback.
        // Remove when the seek bar works properly on all systems.
        self.seeking = false;

        self.request_state(match self.backend.current_state() {
            State::Playing => State::Paused,
            _ => State::Playing,
        });
    }

    /// Removes the item at `index` from the queue
    #[inline]
    fn remove_item(&mut self, index: usize) {
        if index == self.queue.index() {
            if self.next_song_loaded {
                println!("Removing song which was already loaded");
                self.unload_gapless();
                self.queue.remove(index);
                self.ui_set_state();
            }
            self.backend.set_property("instant-uri", true);
            self.queue.pending_track = true;
            self.queue.remove(index);

            self.update();
            self.ui_set_state();
        } else {
            self.queue.remove(index);
        }
    }

    /// Moves to the next track in the queue without flushing the stream
    fn move_next(&mut self, count_played: bool) {
        if count_played {
            // TODO: More advanced play counting (seeking to the end should not count)
            self.queue.current().as_song().info().played();
        }
        self.queue.pending_track = true;
        self.queue.move_next();
    }

    /// Skips to next track
    fn skip_next(&mut self) -> Result<(), gst::StateChangeError> {
        if self.next_song_loaded {
            return self.repeat_song();
        }
        self.backend.set_property("instant-uri", true);
        if !self.queue.get_repeat()
            && self.queue.is_last()
            && !matches!(self.current_state, State::Null)
        {
            self.request_state(State::Ready);
            return Ok(());
        }
        self.move_next(false);
        Ok(())
    }

    /// Skips to the track in the queue at specified `index`
    fn skip_to(&mut self, index: usize) {
        self.backend.set_property("instant-uri", true);
        self.queue.pending_track = true;
        if self.queue.index() == index {
            if matches!(self.backend.current_state(), State::Playing | State::Paused) {
                let _ = self.repeat_song();
            }
            return;
        }
        self.queue.set_index(index);
    }

    /// Skips to previous track
    fn skip_prev(&mut self) {
        self.backend.set_property("instant-uri", true);
        self.queue.pending_track = true;
        self.queue.move_previous();
    }

    /// Seeks to the beginning of the current track
    fn repeat_song(&self) -> Result<(), gst::StateChangeError> {
        self.seek_to_time(ClockTime::ZERO)
    }

    /// Skips to previous track or restarts the current one if above the time threshold
    fn skip_prev_or_repeat(&mut self) -> Result<(), Box<dyn Error>> {
        const REPEAT_THRESHOLD: ClockTime = ClockTime::from_seconds(8);
        if !self.next_song_loaded
            && let Some(time) = self.current_time()
            && (time > REPEAT_THRESHOLD || (self.queue.is_first() && !self.queue.get_repeat()))
        {
            self.repeat_song()?;
        } else {
            self.skip_prev();
        }
        Ok(())
    }

    /// Seek to a position in the song using a 0 to 1 value
    fn seek_to_position(&self, position: f64) -> Result<(), gst::StateChangeError> {
        #[allow(clippy::cast_possible_truncation)]
        #[allow(clippy::cast_precision_loss)]
        #[allow(clippy::cast_sign_loss)]
        let target_ms = ((self.backend.query_duration::<ClockTime>())
            .unwrap_or_default()
            .mseconds() as f64
            * position) as u64;
        self.seek_to_time(ClockTime::from_mseconds(target_ms))
    }

    /// Seek to a particular time in the song
    #[inline]
    fn seek_to_time(&self, time: ClockTime) -> Result<(), gst::StateChangeError> {
        match (self.backend).seek_simple(SeekFlags::FLUSH | SeekFlags::ACCURATE, time) {
            Ok(()) => {
                self.backend.state(ClockTime::from_mseconds(150)).0?;
                self.ui_set_time();
            }
            Err(e) => eprintln!("{e}"),
        }
        Ok(())
    }

    /// Seek to a position in the song using a 0 to 1 value
    /// Remember to call `seek_done()` to resume playback
    fn seek_to_position_paused(&mut self, position: f64) -> Result<(), gst::StateChangeError> {
        self.begin_seek_paused()?;
        self.seek_to_position(position)
    }

    /// Prepare the palyer for interactive seeking in paused state
    /// Remember to call `seek_done()` to resume playback
    fn begin_seek_paused(&mut self) -> Result<(), gst::StateChangeError> {
        // If next track is already loaded, move back to the current one
        if self.next_song_loaded {
            println!("Gapless transition interrupted by seek request");
            self.backend.set_state(State::Null)?;
            self.request_state(self.current_state);
            self.wait_for_gstreamer_state();
            self.next_song_loaded = false;
            self.skip_prev();
            self.update();
            self.wait_for_gstreamer_state();
            self.queue.current().as_song().info().deduct_played();
        }

        match self.backend.current_state() {
            State::Playing => self.backend.set_state(State::Paused).map(|_| ())?,
            State::Paused => (),
            _ => return Ok(()),
        }

        self.seeking = true;
        self.wait_for_gstreamer_state();
        Ok(())
    }

    /// Call to resume the player state when done seeking
    fn seek_done(&mut self) {
        self.seeking = false;

        if let (Some(pos), Some(dur)) = (
            self.backend.query_position::<ClockTime>(),
            (self.backend.query_duration::<ClockTime>()).map(ClockTime::mseconds),
        ) && dur.saturating_sub(pos.mseconds()) != 0
        {
            // Reset state to re-enable missed `GStreamer` callbacks
            let _ = self.backend.set_state(State::Null);
            self.wait_for_gstreamer_state();
            let _ = self.backend.set_state(State::Paused);
            self.wait_for_gstreamer_state();

            // Seek one final time after state reset
            let _ = self.seek_to_time(pos);
        } else {
            // Skip the current song if the song has ended
            // or the playback time/duration cannot be determined
            player_tx().send(PlayerRequest::SkipNext).expect(EXP_RX);
        }

        self.request_state(self.current_state);
    }

    /// Unloads the gaplessly loaded track by restarting the stream
    ///
    /// Note that this might cause an audible stutter, so use it sparingly
    pub fn unload_gapless(&mut self) {
        println!("---- Unloading gapless track ----");
        let Some(pos) = self.backend.query_position::<ClockTime>() else {
            eprintln!("Could not determine playback time, skipping...");
            let _ = player_tx().send(PlayerRequest::SkipNext);
            return;
        };

        let _ = self.backend.set_state(State::Null);
        self.request_state(self.current_state);
        self.wait_for_gstreamer_state();
        self.next_song_loaded = false;
        self.skip_prev();
        self.update();
        self.wait_for_gstreamer_state();
        self.queue.current().as_song().info().deduct_played();

        // Seek to the same time the player was at before, or skip the song
        if self.seek_to_time(pos).is_err() {
            self.queue.current().map_song(|song| song.info().played());
            let _ = player_tx().send(PlayerRequest::SkipNext);
        }
    }

    /// Sets the playback volume
    #[inline]
    fn set_volume(&self, volume: f64) {
        self.backend.set_property("volume", volume);
    }

    /// Moves an item from `from` to `to`
    fn reorder(&mut self, from: usize, to: usize) {
        if self.next_song_loaded
            && (from == self.queue.index() - 1
                || from == self.queue.index()
                || (from < to && to == self.queue.index() - 1)
                || (from > to && to == self.queue.index()))
        {
            self.unload_gapless();
        }
        self.queue.reorder(from, to);
    }

    /// Moves an item at position `from` to the position at `from + by`
    ///
    /// If repeat mode is enabled, the final index will wrap around the
    /// queue if it is out of bounds.
    ///
    /// # Panics
    /// The function panics if the final index is out of bounds and
    /// repeat mode is disabled
    fn shift(&mut self, from: usize, by: isize) {
        let to = from as isize + by;
        if self.queue.get_repeat() {
            // Offsetting the length by -1 because moving items to
            // the first position or last is considered equivalent
            self.reorder(from, wrap_index(to, self.queue.len() - 1));
        } else {
            assert!(
                (0..self.queue.len() as isize).contains(&to),
                "Shifting to an out-of-bounds index ({to}) is only allowed in repeat mode"
            );
            self.reorder(from, to as usize);
        }
    }

    /// Inserts a `QueueItem` into the current queue at the specified `index`
    fn insert_to_queue(&mut self, index: usize, item: QueueItem) {
        if self.next_song_loaded && index == self.queue.index() {
            self.unload_gapless();
        }
        self.queue.insert(index, item);
    }

    /// Sets player state the next time `update()` is called
    #[inline]
    const fn request_state(&mut self, state: State) {
        self.pending_state = Some(state);
    }

    /// Current playback time in the song
    #[inline]
    fn current_time(&self) -> Option<ClockTime> {
        self.backend.query_position::<ClockTime>()
    }

    /// Retruns `true` if a gapless transition is appropriate for the
    /// current state. Always returns `false` if gapless mode is disabled
    fn can_use_gapless(&self) -> bool {
        self.gapless
            && !self.seeking
            && !self.queue.next().is_some_and(QueueItem::is_stopper)
            && (!self.queue.is_last() || self.queue.get_repeat())
    }

    /// Sends the current state to the UI receiver
    #[inline]
    fn ui_set_state(&self) {
        let state = self.backend.state(ClockTime::from_mseconds(150));
        let interactive = !self.queue.is_empty();
        let playing = state.0.is_ok() && matches!(state.1, State::Playing);
        #[cfg(debug_assertions)]
        println!("ui_set_state(playing: {playing}, interactive: {interactive})");
        (ui_tx().send(UpdateUI::PlayerState {
            playing,
            interactive,
        }))
        .expect(EXP_RX);
    }

    /// Sends the current song info to the UI receiver
    #[inline]
    fn ui_update_song_info(&self) {
        #[cfg(debug_assertions)]
        println!("ui_update_song_info()");
        let item = match self.queue.is_empty() {
            false => QueueItem::clone(self.queue.current()),
            true => QueueItem::new_stopper(false),
        };
        let pause_after = self.queue.next().is_some_and(QueueItem::is_stopper);
        (ui_tx().send(UpdateUI::SongInfo { item, pause_after })).expect(EXP_RX);
    }

    /// Sends the current playback time to the UI receiver
    fn ui_set_time(&self) {
        let time = self.current_time().map(ClockTime::mseconds);
        ui_tx().send(UpdateUI::PlayerTime { time }).expect(EXP_RX);
    }

    /// Requests the UI to open the music library
    fn ui_open_playing() {
        let ui_tx = ui_tx();
        ui_tx.send(UpdateUI::FocusPlaying).expect(EXP_RX);
        ui_tx.send(UpdateUI::OpenSheet(true)).expect(EXP_RX);
    }

    fn wait_for_gstreamer_state(&mut self) {
        if (self.backend.state(Some(ClockTime::from_mseconds(150))).0).is_err() {
            self.force_stop_playback();
        }
    }

    /// Handles `GStreamer` events and empties the message queue
    fn handle_gst_events(&mut self) {
        while let Some(message) = self.bus.pop() {
            match message.type_() {
                gst::MessageType::StreamStart => {
                    println!("Song started");
                    self.queue.ui_update_queue_index();
                    self.ui_update_song_info();
                    self.next_song_loaded = false;
                }
                gst::MessageType::Eos if self.seeking => {
                    println!("EOS ignored while seeking");
                }
                gst::MessageType::Eos => {
                    if matches!(self.current_state, State::Null) {
                        return;
                    }
                    if self.queue.has_next() {
                        println!("Moving to next track due to end of stream");
                        self.request_state(State::Playing);
                        player_tx().send(PlayerRequest::LoadNext).expect(EXP_RX);
                    } else {
                        println!("Stopping player due to end of queue");
                        self.queue.current().as_song().info().played();
                        self.request_state(State::Null);
                        self.queue.pending_track = true;
                        self.update();
                        self.ui_set_state();
                    }
                }
                gst::MessageType::Warning => cold({
                    let warning = format!("{message:?}");
                    eprintln!("gstreamer warning: {warning}\n");

                    // Handling for the case where skipping songs too quickly
                    // could overwhelm GStreamer, causing it to hang indefinitely
                    if warning.contains("buffer without a new-segment") {
                        self.force_stop_playback();
                        while self.rx.try_recv().is_ok() {
                            // Clear all pending `PlayerRequest`s
                        }
                    }
                }),
                gst::MessageType::Error => cold({
                    // Update the UI manually, in case the `StreamStart` branch did not run
                    self.queue.ui_update_queue_index();
                    self.ui_update_song_info();

                    let error = format!("{message:?}");
                    eprintln!("gstreamer error: {error}\n");

                    if error.contains(&self.backend.property::<String>("uri")) {
                        self.force_stop_playback();
                    }
                }),
                _ => (),
            }
        }
    }

    /// Sets the player to `Null` state and shows an error in the UI
    #[cold]
    fn force_stop_playback(&mut self) {
        let _ = self.backend.set_state(State::Null);
        self.current_state = State::Null;
        self.pending_state = None;
        self.next_song_loaded = false;

        let ui_tx = ui_tx();
        let _ = ui_tx.send(UpdateUI::PlayerState {
            playing: false,
            interactive: true,
        });
        let _ = ui_tx.send(UpdateUI::DismissNotifications);
        let _ = ui_tx.send(UpdateUI::Notification(
            "A playback error has occurred".to_owned(),
            None, // IDEA: "Details" button to show the full error message in a pop-up window
        ));
    }

    /// Uninitializes the player and writes the queue to disk
    fn shutdown(self, save_queue: bool, save_time: bool) {
        let time_ms = match self.current_time().map(ClockTime::mseconds) {
            Some(time_ms) if time_ms > 999 && save_time => Some(time_ms),
            _ => None,
        };
        self.queue.save_queue(save_queue, time_ms);
        self.queue.save_shuffled_queue(save_queue);
        let _ = self.backend.set_state(State::Null);
    }
}
