// IDEA: Could `GIO` be used instead of the `mpris-server` crate?

use gio::prelude::FileExt;
use gtk::{gio, glib};
use mpris_server::{self, LoopStatus, Metadata, PlaybackStatus, zbus};
use std::sync::OnceLock;
use tokio::sync::mpsc as tokio_mpsc;

use crate::about::app_id;
use crate::excuses::EXP_RX;
use crate::player::{PlayerRequest, QueueItem, player_tx};
use crate::ui::{UpdateUI, ui_tx};

static MPRIS_TX: OnceLock<tokio_mpsc::UnboundedSender<UpdateMPRIS>> = OnceLock::new();
/// Returns the channel sender for sending requests to the MPRIS interface using `UpdateMPRIS`
///
/// # Safety
/// Causes undefined behavior if called before `init_channels`
#[inline]
pub fn mpris_tx() -> &'static tokio_mpsc::UnboundedSender<UpdateMPRIS> {
    // SAFETY: `init_channels` runs in `Application::run`, before starting any threads
    unsafe { MPRIS_TX.get().unwrap_unchecked() }
}
/// Initializes the mpris channel sender accessed through `mpris_tx()`
///
/// # Errors
/// The function returns an error if `MPRIS_TX` has already been initialized
#[inline]
pub fn init_mpris_tx(
    mpris_tx: tokio_mpsc::UnboundedSender<UpdateMPRIS>,
) -> Result<(), tokio_mpsc::UnboundedSender<UpdateMPRIS>> {
    MPRIS_TX.set(mpris_tx)
}

pub enum UpdateMPRIS {
    SongInfo(QueueItem),
    PlayState(bool),
    Shuffle(bool),
    Repeat(bool),
}

/// Creates the MPRIS interface and updates it
///
/// # Errors
/// Propagates any errors from the `mpris-server` crate
/// during initialization and when updating the metadata
///
/// # Panics
/// May panic if the player or UI channel is closed
pub async fn controller(mut rx: tokio_mpsc::UnboundedReceiver<UpdateMPRIS>) -> zbus::Result<()> {
    let mpris_player = mpris_server::Player::builder(app_id())
        .identity("Mellow")
        .can_play(true)
        .can_pause(true)
        .can_go_previous(true)
        .can_go_next(true)
        .can_raise(true)
        .can_quit(true)
        .build()
        .await?;

    mpris_player.connect_play_pause(|_| {
        player_tx()
            .send(PlayerRequest::TogglePlay(None))
            .expect(EXP_RX);
    });
    mpris_player.connect_previous(|_| player_tx().send(PlayerRequest::SkipPrevious).expect(EXP_RX));
    mpris_player.connect_next(|_| player_tx().send(PlayerRequest::SkipNext).expect(EXP_RX));
    mpris_player.connect_quit(|_| ui_tx().send(UpdateUI::Shutdown).expect(EXP_RX));
    mpris_player.connect_raise(|_| {
        ui_tx()
            .send(UpdateUI::RunAction("app.show_window"))
            .expect(EXP_RX);
    });

    glib::spawn_future_local(mpris_player.run());

    loop {
        match rx.recv().await.unwrap() {
            UpdateMPRIS::SongInfo(QueueItem::Song(song)) => {
                let mut info = song.info();
                let mut metadata = {
                    // The scope ensures the `MutexGuard` is dropped before `await`
                    // (explicit `drop` still triggers the `clippy::await_holding_lock` warning)
                    let basic_info = info.load_basic();
                    let basic_info = basic_info.as_ref().unwrap();
                    Metadata::builder()
                        .title(&basic_info.title)
                        .album(&basic_info.album)
                        .artist([&basic_info.artist])
                        .build()
                };
                metadata.set_art_url(match info.inspect_thumbnail().is_some() {
                    true => Some(gio::File::for_path(info.thumbnail_file_path()).uri()),
                    false => None,
                });
                mpris_player.set_metadata(metadata).await?;
                mpris_player.set_can_play(true).await?;
            }
            UpdateMPRIS::SongInfo(QueueItem::Stopper(_)) => {
                mpris_player.set_can_play(false).await?;
            }
            UpdateMPRIS::PlayState(playing) => {
                mpris_player
                    .set_playback_status(match playing {
                        true => PlaybackStatus::Playing,
                        false => PlaybackStatus::Paused,
                    })
                    .await?;
            }
            UpdateMPRIS::Shuffle(shuffle) => mpris_player.set_shuffle(shuffle).await?,
            UpdateMPRIS::Repeat(repeat) => {
                mpris_player
                    .set_loop_status(match repeat {
                        true => LoopStatus::Playlist,
                        false => LoopStatus::None,
                    })
                    .await?;
            }
        }
    }
}
