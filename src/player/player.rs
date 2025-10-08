use core::error::Error;
use gst::prelude::{ElementExt, ElementExtManual, ObjectExt};
use gst::{ClockTime, SeekFlags, State};
use rand::random_range;
use std::sync::mpsc;
use std::thread;
use std::time::Duration;

use crate::library::Song;

// TODO: MPRIS support for Gnome Shell media controls

#[derive(Debug)]
pub enum PlayerRequest {
    /// Play or pause depending on the current state
    PlayOrPause,
    /// Skip to the next song in the queue
    SkipNext,
    /// Skip to beginning or previous song
    SkipPrevious,
    /// Seek to a particular point in the song using a 0 to 1 value
    Seek(f64),
    /// Used internally to signal when song is about to end
    SongEnd,

    /// Send the current state to `ui_rx`
    GetCurrentState,
    /// Send the current time to `ui_rx`
    GetCurrentTime,
}

pub enum PlayerResponse {
    State(State),
    Time(Option<ClockTime>),
    TrackChanged,
}

pub struct Player {
    pub song_index: usize,
    pub queue: Vec<Song>,
    pub repeat: bool,
    pub shuffle: bool, // TODO: Button to randomize the queue instead of shuffle mode

    state: State,
    pending_track: bool,
    backend: gst::Element,
    bus: gst::Bus,
    ui_tx: mpsc::SyncSender<PlayerResponse>,
    rx: mpsc::Receiver<PlayerRequest>,
}

// NOTE: Set `GST_DEBUG=3` to debug GStreamer
// https://gstreamer.freedesktop.org/documentation/tutorials/basic/debugging-tools.html

impl Player {
    pub fn init() -> Result<
        (
            Player,
            mpsc::SyncSender<PlayerRequest>,
            mpsc::Receiver<PlayerResponse>,
        ),
        Box<dyn Error>,
    > {
        gst::init().unwrap();

        let playbin = gst::ElementFactory::make("playbin3").build()?;
        let bus = playbin.bus().unwrap();

        let (player_tx, rx) = mpsc::sync_channel::<PlayerRequest>(2);
        let (ui_tx, ui_rx) = mpsc::sync_channel::<PlayerResponse>(0);

        Ok((
            Player {
                song_index: 0,
                queue: vec![],
                repeat: false,
                shuffle: false,

                state: State::Null,
                pending_track: true,
                backend: playbin,
                bus,
                ui_tx,
                rx,
            },
            player_tx,
            ui_rx,
        ))
    }

    /// Handles playback controls and responds to requests
    pub fn event_handler(
        &mut self,
        player_tx: mpsc::SyncSender<PlayerRequest>,
    ) -> Result<(), Box<dyn Error>> {
        self.backend.connect("about-to-finish", false, move |_| {
            player_tx.send(PlayerRequest::SongEnd).unwrap();
            None
        });

        loop {
            self.clear_gst_msg_queue();
            match self.rx.recv()? {
                PlayerRequest::SongEnd => self.move_next(),
                PlayerRequest::PlayOrPause => self.play_or_pause(),
                PlayerRequest::SkipPrevious => self.skip_prev_or_repeat()?,
                PlayerRequest::Seek(pos) => self.seek_to_position(pos)?,
                PlayerRequest::SkipNext => self.skip_next(),

                PlayerRequest::GetCurrentTime => {
                    self.ui_tx.send(PlayerResponse::Time(self.current_time()))?;
                    continue;
                }
                PlayerRequest::GetCurrentState => {
                    self.ui_tx.send(PlayerResponse::State(self.state))?;
                    continue;
                }
            };

            self.update()?;
        }
    }

    /// Clears the GStreamer message queue and prints out errors/warnings/EOS
    fn clear_gst_msg_queue(&self) {
        while let Some(message) = self.bus.pop() {
            match message.type_() {
                gst::MessageType::Error => eprintln!("gstreamer error: {message:?}\n"),
                gst::MessageType::Warning => eprintln!("gstreamer warning: {message:?}\n"),
                gst::MessageType::Eos => println!("gstreamer: End-Of-Stream\n"),
                _ => (),
            }
        }
    }

    // /// Blocks until EOS is signaled by GStreamer
    // /// Does nothing if not currently playing
    // fn wait_for_eos(&self) {
    //     if self.backend.current_state() != State::Playing {
    //         return;
    //     }
    //     loop {
    //         if self.bus.pop_filtered(&[gst::MessageType::Eos]).is_some() {
    //             break;
    //         }
    //     }
    // }

    fn update(&mut self) -> Result<(), Box<dyn Error>> {
        self.clear_gst_msg_queue();

        if self.queue.is_empty() {
            if self.repeat {
                self.restart_queue();
            } else {
                self.state = State::Null;
                self.backend.set_state(self.state)?;
                return Ok(());
            }
        }

        // dbg!(&self.history.len());
        // dbg!(&self.queue.len());

        let song = &mut self.queue[self.song_index];

        if self.pending_track {
            println!("{}", song.file_uri());
            self.backend.set_property("uri", song.file_uri());
        }

        self.backend.set_state(self.state)?;

        // TODO: Gracefully handle state switch errors (for example: missing plugins)
        // self.backend.state(None).0?;
        // if self.backend.state(None).0.is_err() {
        //     eprintln!("Failed to set GStreamer state\nSkipping song");
        //     let repeat = self.repeat;
        //     self.repeat = false;
        //     self.skip_next();
        //     self.repeat = repeat;
        //     return self.update();
        // }

        if self.pending_track {
            // Wait for last track to finish playing
            // WARN: This will block the entire thread, which means that
            //       the UI will not be able to communicate with it until
            //       the track change is complete. If it waits for an
            //       answer, it will block the UI as well. Time position
            //       queries would need to be substituted with own logic.
            //       It also blocks the skip/pause buttons until the song
            //       starts, which is not good user experience.
            //       However, not waiting would mean that the UI would
            //       change to the next song before the current one is
            //       finished playing.
            // FIX: Stuck waiting for song to end
            println!("Next song is ready");
            while self
                .backend
                .query_position()
                .unwrap_or_else(|| ClockTime::from_seconds(0))
                > ClockTime::from_seconds(2)
            {
                thread::sleep(Duration::from_millis(20));
            }

            // TODO: Find a way to efficiently communicate song info to the UI
            song.info.take();

            let properties = song.get_info_or_assign();

            println!();
            println!("Title: {}", properties.title);
            println!("Album: {}", properties.album);
            println!("Artist: {}", properties.artist);
            println!("Album Artist: {}", properties.album_artist);
            println!("Track: {}", properties.track);
            println!("Year: {}", properties.year);
            // println!("\nLyrics:\n{}\n", properties.lyrics);

            println!("Duration: {}\n", properties.duration);
        }

        // Re-enable gapless playback (for example after track skip)
        self.backend.set_property("instant-uri", false);

        self.pending_track = false;

        Ok(())
    }

    pub fn current_time(&self) -> Option<ClockTime> {
        self.backend.query_position::<ClockTime>()
    }

    pub fn song_duration(&self) -> Option<ClockTime> {
        if self.queue.is_empty() {
            return None;
        }
        self.queue[0].info.as_ref().map(|info| info.duration)

        // Duration from gstreamer seems unreliable
        // self.backend.query_duration::<ClockTime>()
    }

    /// Replaces the current queue with the provided one
    pub fn new_queue(&mut self, queue: Vec<Song>) {
        self.backend.set_property("instant-uri", true);
        self.pending_track = true;
        self.queue = queue;
    }

    /// Restarts the queue from the beginning
    pub fn restart_queue(&mut self) {
        self.backend.set_property("instant-uri", true);
        self.pending_track = true;
        self.song_index = 0;
    }

    /// Randomizez the order of songs in the queue
    /// Playback state has to be manually updated
    pub fn randomize_queue(&mut self) {
        self.backend.set_property("instant-uri", true);
        self.pending_track = true;
        for i in 0..self.queue.len() {
            let rand_index = random_range(0..self.queue.len());
            self.queue.swap(i, rand_index);
        }
        self.song_index = 0;
    }

    /// Removes all upcomming songs from the queue
    pub fn clear_queue(&mut self) {
        let current_song = self.queue.remove(0);
        self.queue = vec![current_song];
    }

    /// Removes all queued songs after the provided index
    pub fn clear_queue_after_index(&mut self, index: usize) {
        while self.queue.len() > index + 1 {
            self.queue.remove(index + 1);
        }
    }

    fn play_or_pause(&mut self) {
        self.state = match self.backend.current_state() {
            State::Playing => State::Paused,
            State::Paused => State::Playing,
            State::Ready => State::Playing,
            _ => State::Playing,
        }
    }

    // FIX: Need to press 3 times to skip back a paused song when over 5 seconds
    // It looks like `current_clock_time()` remains outdated while paused
    fn skip_prev_or_repeat(&mut self) -> Result<(), Box<dyn Error>> {
        let current_time = self.backend.current_clock_time();
        if let Some(time) = current_time
            && (time > ClockTime::from_seconds(10) || (self.song_index == 0 && !self.repeat))
        {
            return self.repeat_song();
        }

        self.skip_prev();
        Ok(())
    }

    fn repeat_song(&self) -> Result<(), Box<dyn Error>> {
        self.backend.set_state(State::Playing)?; // Can't seek while paused..?
        self.backend.seek_simple(
            SeekFlags::FLUSH | SeekFlags::ACCURATE | SeekFlags::TRICKMODE_NO_AUDIO,
            ClockTime::from_seconds(0),
        )?;
        self.backend.set_state(State::Ready)?;
        Ok(())
    }

    /// Seek to a position in the song using a 0 to 1 value
    fn seek_to_position(&self, position: f64) -> Result<(), Box<dyn Error>> {
        let target_ms = (self
            .backend
            .query_duration::<ClockTime>()
            .unwrap_or_else(|| ClockTime::from_seconds(0))
            .mseconds() as f64
            * position) as u64;
        // println!("Target seek position (ms): {target_ms}");
        self.seek_to_time(ClockTime::from_mseconds(target_ms))
    }

    /// Seek to a particular time in the song
    fn seek_to_time(&self, time: ClockTime) -> Result<(), Box<dyn Error>> {
        self.backend
            .seek_simple(SeekFlags::FLUSH | SeekFlags::ACCURATE, time)?;
        Ok(())
    }

    fn skip_prev(&mut self) {
        self.backend.set_property("instant-uri", true);
        self.pending_track = true;
        if self.song_index == 0 {
            if self.repeat {
                self.song_index = self.queue.len() - 1;
            }
            return;
        }
        self.song_index -= 1;
    }

    fn skip_next(&mut self) {
        self.backend.set_property("instant-uri", true);
        self.move_next()
    }

    fn move_next(&mut self) {
        self.pending_track = true;
        if self.song_index == self.queue.len() - 1 {
            if self.repeat {
                self.song_index = 0;
            } else {
                self.pending_track = false;
                self.state = State::Null;
            }
            return;
        }
        self.song_index += 1;
    }
}
