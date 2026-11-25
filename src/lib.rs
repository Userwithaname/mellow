use std::sync::mpsc;

use crate::library::{LibraryRequest, init_library_tx};
use crate::mpris::{UpdateMPRIS, init_mpris_tx};
use crate::player::{PlayerRequest, init_player_tx};
use crate::ui::{UpdateUI, init_ui_tx};

pub mod about;
pub mod excuses;
pub mod library;
pub mod mpris;
pub mod player;
pub mod shortcuts;
pub mod ui;
pub mod util;

type ChannelReceivers = (
    mpsc::Receiver<PlayerRequest>,
    mpsc::Receiver<LibraryRequest>,
    async_channel::Receiver<UpdateUI>,
    async_channel::Receiver<UpdateMPRIS>,
);
#[derive(Debug)]
pub struct AlreadyInitializedError;

/// Initializes the component channels and returns their receivers
///
/// The returned channel receivers should be passed to their respective
/// components' `init` functions
///
/// # Errors
/// Function returns an error if the channels were already initialized
#[inline]
#[must_use = "Channel receivers should be passed to their respective components' `init` functions"]
pub fn init_channels() -> Result<ChannelReceivers, AlreadyInitializedError> {
    let (player_tx, player_rx) = mpsc::channel::<PlayerRequest>();
    let (library_tx, library_rx) = mpsc::channel::<LibraryRequest>();
    let (ui_tx, ui_rx) = async_channel::unbounded::<UpdateUI>();
    let (mpris_tx, mpris_rx) = async_channel::unbounded::<UpdateMPRIS>();

    init_player_tx(player_tx).map_err(|_| AlreadyInitializedError)?;
    let _ = init_library_tx(library_tx);
    let _ = init_ui_tx(ui_tx);
    let _ = init_mpris_tx(mpris_tx);

    Ok((player_rx, library_rx, ui_rx, mpris_rx))
}
