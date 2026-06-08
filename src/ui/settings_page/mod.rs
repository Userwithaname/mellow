use adw::{prelude::*, subclass::prelude::*};
use core::cell::Ref;
use gtk::{gdk, glib};

use crate::excuses::INIT_ERR;

mod imp;

glib::wrapper! {
    pub struct SettingsPage(ObjectSubclass<imp::SettingsPage>)
        @extends adw::PreferencesPage, gtk::Widget,
        @implements
            gtk::Accessible, gtk::Actionable, gtk::Buildable, gtk::Orientable, gtk::ConstraintTarget;
}

#[derive(Default, Debug, Copy, Clone)]
pub enum StartupQueueChoice {
    #[default]
    RestoreQueue = 0,
    QueueFromSongs = 1,
    QueueFromAlbums = 2,
    QueueFromArtists = 3,
    QueueFromSongsShuffled = 4,
    QueueFromAlbumsShuffled = 5,
    QueueFromArtistsShuffled = 6,
    EmptyQueue = 7,
}
impl From<i32> for StartupQueueChoice {
    fn from(value: i32) -> Self {
        match value {
            0 => Self::RestoreQueue,
            1 => Self::QueueFromSongs,
            2 => Self::QueueFromAlbums,
            3 => Self::QueueFromArtists,
            4 => Self::QueueFromSongsShuffled,
            5 => Self::QueueFromAlbumsShuffled,
            6 => Self::QueueFromArtistsShuffled,
            7 => Self::EmptyQueue,
            n => {
                eprintln!(
                    "WARNING: Value {n} is outside the valid range for `StartupQueueChoice` (default value will be used instead)"
                );
                Self::default()
            }
        }
    }
}

impl SettingsPage {
    /// Initializes various components, such as for theming support
    ///
    /// The `style_main` and `style_menu` widgets will be styled
    /// accordingly when adaptive colors are enabled
    ///
    /// # Panics
    /// The function panics if called when already initialized
    #[inline]
    pub fn init(
        &self,
        style_manager: adw::StyleManager,
        style_main: Vec<gtk::Widget>,
        style_menu: Vec<gtk::Widget>,
        style_soft: Vec<gtk::Widget>,
    ) {
        let imp = self.imp();
        // TODO: Detect system color scheme
        // let style_preference = style_manager.color_scheme();
        let _ = imp.css.set(gtk::CssProvider::new());
        let css = imp.css.get().expect(INIT_ERR);
        let _ = imp.style_manager.set(style_manager);
        imp.set_theme(adw::ColorScheme::ForceDark);
        if let Some(display) = gdk::Display::default() {
            gtk::style_context_add_provider_for_display(&display, css, 210);
        }
        imp.style_main.replace(style_main);
        imp.style_menu.replace(style_menu);
        imp.style_soft.replace(style_soft);
    }

    /// Returns the current value of the volume slider
    #[inline]
    #[must_use]
    pub fn volume(&self) -> f64 {
        self.imp().volume.value()
    }
    /// Sets the volume slider to the provided value;
    /// note that this does not update the player volume
    #[inline]
    pub fn set_volume(&self, volume: f64) {
        self.imp().volume.set_value(volume);
    }

    /// Returns the current gapless mode setting
    #[inline]
    #[must_use]
    pub fn gapless(&self) -> bool {
        self.imp().gapless.is_active()
    }
    /// Enables or disables the gapless mode
    #[inline]
    pub fn set_gapless(&self, gapless: bool) {
        self.imp().gapless.set_active(gapless);
    }

    /// Returns the current background playback setting
    #[inline]
    #[must_use]
    pub fn play_in_background(&self) -> bool {
        self.imp().play_in_background.is_active()
    }
    /// Enables or disables background playback
    #[inline]
    pub fn set_play_in_background(&self, play_in_background: bool) {
        self.imp().play_in_background.set_active(play_in_background);
    }

    /// Returns the currently selected startup queue preference
    #[inline]
    #[must_use]
    pub fn startup_queue(&self) -> Ref<'_, StartupQueueChoice> {
        self.imp().startup_choice.borrow()
    }
    /// Sets the startup queue preference to the specified choice
    #[inline]
    pub fn set_startup_queue(&self, choice: StartupQueueChoice) {
        let settings = self.imp();
        settings.startup_choice.replace(choice);
        match choice {
            StartupQueueChoice::RestoreQueue => settings.remember_queue.set_active(true),
            StartupQueueChoice::EmptyQueue => settings.empty_queue.set_active(true),
            _ => {
                settings.new_queue.set_active(true);
                settings.shuffle_queue.set_active(choice as i32 > 3);
                settings.queue_source.set_selected(match choice as u32 - 1 {
                    source if source > 2 => source - 3,
                    source => source,
                });
            }
        }
    }

    /// Returns `true` if the startup choice is `RestoreQueue`,
    /// otherwise returns `false`
    #[inline]
    #[must_use]
    pub fn remembers_queue(&self) -> bool {
        matches!(
            *self.imp().startup_choice.borrow(),
            StartupQueueChoice::RestoreQueue
        )
    }
    /// Whether the player remembers the playback time
    /// Note that this setting only applies when the startup
    /// queue is set to `RestoreQueue`
    #[inline]
    #[must_use]
    pub fn remembers_time(&self) -> bool {
        self.imp().remember_time.is_active()
    }
    /// Sets whether time should be restored along with the
    /// queue when `RestoreQueue` is selected
    #[inline]
    pub fn set_remember_time(&self, remember_time: bool) {
        self.imp().remember_time.set_active(remember_time);
    }

    /// Returns `true` if adaptive colors are enabled,
    /// otherwise returns `false`
    #[inline]
    #[must_use]
    pub fn adaptive_colors(&self) -> bool {
        self.imp().adaptive_colors.is_active()
    }
    /// Enables or disables adaptive colors
    #[inline]
    pub fn set_adaptive_colors(&self, adaptive_colors: bool) {
        self.imp().adaptive_colors.set_active(adaptive_colors);
    }

    /// Returns the currently selected color scheme as `u32`
    ///
    /// The values are currently hardcoded as such:
    /// - 0: Dark
    /// - 1: Light
    /// - 2: Auto
    #[inline]
    #[must_use]
    pub fn color_scheme(&self) -> u32 {
        self.imp().color_scheme.selected()
    }
    /// Sets the application color scheme
    ///
    /// Valid values are as follows:
    /// - 0: Dark
    /// - 1: Light
    /// - 2: Auto
    #[inline]
    pub fn set_color_scheme(&self, id: u32) {
        self.imp().color_scheme.set_selected(id);
    }

    /// Returns the list of directories shown in the UI
    #[inline]
    #[must_use]
    pub fn directories(&self) -> Vec<String> {
        self.imp().directories.borrow().clone()
    }
    /// Sets the directories displayed by the UI;
    /// note that this does not affect the library
    /// configuration
    #[inline]
    pub fn set_directories(&self, directories: &[String]) {
        self.imp().set_directories(directories);
    }

    /// Whether to allow the library refresh button to be pressed
    #[inline]
    pub fn allow_library_refresh(&self, allow: bool) {
        self.imp().refresh_library_button.set_sensitive(allow);
    }

    /// Whether to allow adding or removing directories through the UI
    #[inline]
    pub fn allow_library_changes(&self, allow: bool) {
        self.imp().allow_library_changes(allow);
    }

    /// Resets the adaptive background color and show the default
    /// background instead; useful when an album cover is missing
    #[inline]
    pub fn reset_background_color(&self) {
        self.imp().reset_background_color();
    }
    /// Sets the adaptive background color and tries to match the
    /// colors in the artwork. If adaptive colors are disabled, the
    /// color will still be stored in memory, so it is ready to use
    /// in case the user chooses to enable adaptive colors.
    #[inline]
    pub fn set_background_from_artwork(&self, artwork: &gdk::Texture) {
        self.imp().set_background_from_artwork(artwork);
    }
}
