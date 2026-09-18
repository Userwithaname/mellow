use std::{fs, panic::PanicHookInfo, process, thread};

use crate::about::APP_URL;
use crate::{queue_file, shuffled_queue_file, ui::UpdateUI};

/// A panic hook (see `std::panic::hook`) to ensure the process exits completely
/// if a single thread panics, and also handles some specific cases as well
///
/// If possible, a crash dialog is presented before closing. This is handled
/// through `UpdateUI::CrashNotice`, which attempts to cleanly shutdown all
/// unaffected components once the button is pressed
///
/// If a message cannot be displayed, the process is stopped directly using
/// `std::process::exit(1)` instead
pub(super) fn handle_crash(info: &PanicHookInfo, ui_tx: &async_channel::Sender<UpdateUI>) {
    let location = match info.location() {
        Some(location) => format!(
            "{}@{}:{}",
            location.file(),
            location.line(),
            location.column()
        ),
        None => "(unknown)".to_owned(),
    };
    let thread = thread::current();
    let thread_name = thread.name().unwrap_or_default();
    let panic_reason = info.payload_as_str().unwrap_or_default();

    let mut info = format!("Thread `{thread_name}` panicked at {location}:\n{panic_reason}\n\n");
    match thread_name {
        "player" => handle_player_crash(&mut info),
        _ => append_issues_link(&mut info),
    }

    eprintln!("{info}\n");
    if ui_tx.send_blocking(UpdateUI::CrashNotice(info)).is_err() {
        process::exit(1);
    }
}

fn append_issues_link(info: &mut String) {
    info.push_str(&format!("Please report this issue on {APP_URL}/issues"));
}

fn handle_player_crash(info: &mut String) {
    // Remove invalid indexes from the `shuffled_queue` file so it doesn't crash on next launch
    // (a `queue` file out-of-bounds index is handled on load, so this is not handled here)
    if info.contains("out of bounds")
        && let Ok(queue) = fs::read_to_string(queue_file())
        && let queue_len = queue.lines().skip(4).count()
        && let Ok(shuffled) = fs::read_to_string(shuffled_queue_file())
        && let mut invalid_count = 0
        && let shuffled = (shuffled.lines())
            .filter_map(|line| match line.parse::<usize>() {
                Ok(index) if index < queue_len => Some(index.to_string() + "\n"),
                _ => {
                    invalid_count += 1;
                    None
                }
            })
            .collect::<String>()
        && invalid_count > 0
    {
        if fs::write(shuffled_queue_file(), shuffled).is_ok() {
            return info.push_str(&format!(
                "{invalid_count} invalid item(s) have been removed from the shuffled queue\
                \n\nIf you did not manually edit the `queue` or `shuffled_queue` files, or if the \
                issue persists, please report this issue on {APP_URL}/issues"
            ));
        }
    }
    append_issues_link(info);
}
