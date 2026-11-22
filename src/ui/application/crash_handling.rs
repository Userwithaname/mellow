use adw::prelude::*;
use gtk::{gio, glib};
use std::{fs, panic::PanicHookInfo, process, thread};

use crate::about::APP_URL;
use crate::excuses::ACTION_ERR;
use crate::{queue_file, shuffled_queue_file, ui::UpdateUI};

/// A panic hook (see `std::panic::hook`) to ensure the process exits completely
/// if a single thread panics, and also handles some specific cases as well
///
/// Attempts to present a crash dialog before closing (handled through
/// `UpdateUI::CrashNotice`; see also: `show_crash_dialog`), otherwise the
/// process is stopped directly using `std::process::exit(1)` instead
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

    let mut info = format!("Thread `{thread_name}` panicked at {location}:\n{panic_reason}");
    #[allow(clippy::single_match, reason = "More branches may be added later")]
    match thread_name {
        "player" => handle_player_crash(&mut info),
        _ => (),
    }

    eprintln!("{info}\n");
    if ui_tx.send_blocking(UpdateUI::CrashNotice(info)).is_err() {
        process::exit(1);
    }
}

/// Presents an error to the user and prevents any further interaction
/// with the application
///
/// Pressing the "Quit" button attempts to cleanly shutdown any unaffected
/// components
#[cold]
#[inline(never)]
pub fn show_crash_dialog(error_info: &str, parent: gtk::Widget) {
    // GitHub issue link documentation:
    // https://docs.github.com/en/issues/tracking-your-work-with-issues/using-issues/creating-an-issue#creating-an-issue-from-a-url-query
    let bug_report_link = format!(
        "{APP_URL}/issues/new?tags=bug&body={}",
        glib::Uri::escape_string(
            &format!(
                "Steps leading up to the crash (please describe if possible):\n1. …\n\n\
                 Mellow crashed with the following error:\n```\n{error_info}\n```"
            ),
            None,
            false,
        )
        .replace("%20", "+")
    );
    let link_button = gtk::LinkButton::builder()
        .label("Click here to report this issue on GitHub")
        .uri(bug_report_link)
        .build();
    let dialog = adw::AlertDialog::builder()
        .heading("A component has crashed")
        .body(error_info)
        .extra_child(&link_button)
        .build();
    dialog.add_responses(&[("", "Quit")]);
    dialog.choose(Some(&parent.clone()), gio::Cancellable::NONE, move |_| {
        parent.activate_action("app.quit", None).expect(ACTION_ERR);
    });
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
        && fs::write(shuffled_queue_file(), shuffled).is_ok()
    {
        info.push_str(&format!(
            "{invalid_count} invalid item(s) have been removed from the shuffled queue;\n\
            If the issue persists, please consider reporting it using the link below"
        ));
    }
}
