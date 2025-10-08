use std::thread;

use mellow::app;
use mellow::library::{Library, Song};
use mellow::player::Player;

fn main() {
    let (mut player, player_tx, ui_rx) = Player::init().expect("Failed to initialize player");

    // IDEA: Define `SongInfo` and `gst::State` as `Arc<Mutex<_>>` and create
    // a clone of each, then send one to Player and the other to the UI. This
    // would allow easy communication with the UI. Might be worth it to learn
    // Relm4 properly before attempting to come up with solutions, though.

    thread::Builder::new()
        .name("player".to_string())
        .spawn({
            let player_tx = player_tx.clone();
            move || {
                init_player_queue(&mut player);
                player
                    .event_handler(player_tx)
                    .expect("Player thread crashed")
            }
        })
        .unwrap();

    app::run((player_tx, ui_rx));
}

fn init_player_queue(player: &mut Player) {
    let mut args = std::env::args();
    args.next();
    if args.len() > 0 {
        player.new_queue(
            args.filter_map(|file| Song::new(&file, None).ok())
                .collect(),
        );
    } else {
        let library = Library::rebuild().unwrap();
        player.shuffle = true;
        player.new_queue(library.songs);
        player.randomize_queue();
    }
}
