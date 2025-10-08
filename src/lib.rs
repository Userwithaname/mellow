use std::time::Duration;

pub mod app;
pub mod library;
pub mod player;

pub use library::*;
pub use player::*;

pub fn format_duration(duration: &Duration) -> String {
    let duration = duration.as_secs();
    let seconds = duration % 60;
    format!(
        "{}:{}{seconds}",
        (duration - seconds) / 60,
        if seconds < 10 { "0" } else { "" }
    )
}
