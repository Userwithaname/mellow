use adw::{ShortcutsItem, ShortcutsSection, prelude::*};
use glib::object::IsA;
use gtk::glib;

use crate::ui::Application;

pub trait Shortcuts {
    fn setup_shortcuts(&self);
}
impl Shortcuts for Application {
    #[inline]
    fn setup_shortcuts(&self) {
        // Player Shortcuts
        self.set_accels_for_action("player.play_pause", &["<Ctrl>P"]);
        self.set_accels_for_action("player.skip_prev", &["<Ctrl>Left", "<Ctrl>Up"]);
        self.set_accels_for_action("player.skip_next", &["<Ctrl>Right", "<Ctrl>Down"]);
        self.set_accels_for_action("win.queue_from_disk", &["<Ctrl>O"]);
        // TODO: Ignore shortcut when the overlay is open
        // self.set_accels_for_action("player.play_pause", &["space"]);

        // Overlay Shortcuts
        self.set_accels_for_action("ui.toggle_sheet", &["<Ctrl>L"]);
        self.set_accels_for_action("ui.open_library", &["<Ctrl><Shift>L"]);
        self.set_accels_for_action("ui.open_playing", &["<Ctrl><Shift>P", "<Ctrl>period"]);
        self.set_accels_for_action("ui.open_settings", &["<Ctrl><Shift>S", "<Ctrl>comma"]);
        self.set_accels_for_action("ui.library_search", &["<Ctrl>F"]);

        // Application Shortcuts
        self.set_accels_for_action("win.show_shortcuts_dialog", &["<Ctrl>question"]);
        self.set_accels_for_action("window.close", &["<Ctrl>W"]);
        self.set_accels_for_action("app.quit", &["<Ctrl>Q"]);
    }
}

/// Creates and opens a new 'Shortcuts' window
pub fn show_shortcuts_dialog(parent: &impl IsA<gtk::Widget>) {
    let shortcuts = adw::ShortcutsDialog::new();

    let player_section = ShortcutsSection::new(Some("Player"));
    player_section.add(ShortcutsItem::new("Play/Pause", "<Ctrl>P"));
    player_section.add(ShortcutsItem::new("Previous Track", "<Ctrl>Left"));
    player_section.add(ShortcutsItem::new("Next Track", "<Ctrl>Right"));
    player_section.add(ShortcutsItem::new("Open Files", "<Ctrl>O"));
    shortcuts.add(player_section);

    let overlay_section = ShortcutsSection::new(Some("Overlay"));
    overlay_section.add(ShortcutsItem::new("Open/Close Overlay", "<Ctrl>L"));
    overlay_section.add(ShortcutsItem::new("Show Library Tab", "<Ctrl><Shift>L"));
    overlay_section.add(ShortcutsItem::new("Show Playing Tab", "<Ctrl><Shift>P"));
    overlay_section.add(ShortcutsItem::new("Show Settings Tab", "<Ctrl>comma"));
    overlay_section.add(ShortcutsItem::new("Focus Search Field", "<Ctrl>F"));
    shortcuts.add(overlay_section);

    let application_section = adw::ShortcutsSection::new(Some("Application"));
    application_section.add(ShortcutsItem::new("Show Shortcuts", "<Ctrl>question"));
    application_section.add(ShortcutsItem::new("Close Window", "<Ctrl>W"));
    application_section.add(ShortcutsItem::new("Quit", "<Ctrl>Q"));
    shortcuts.add(application_section);

    shortcuts.present(Some(parent));
}
