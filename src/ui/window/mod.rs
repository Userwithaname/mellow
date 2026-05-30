use adw::{prelude::*, subclass::prelude::*};
use gdk::{DragAction, FileList};
use gio::Settings;
use glib::Object;
use gtk::{Orientation, gdk, gio, glib};

use crate::excuses::{EXP_INIT, EXP_RX};
use crate::library::{Library, LibraryConfig, LibraryRequest, library_tx};
use crate::player::{PlayerRequest, player_tx};
use crate::ui::{Application, UpdateUI, actions::WindowActions, ui_tx};
use crate::util::serialize_list;

pub mod imp;

glib::wrapper! {
    pub struct Window(ObjectSubclass<imp::Window>)
        @extends adw::ApplicationWindow, gtk::ApplicationWindow, gtk::Window, gtk::Widget,
        @implements
            gio::ActionGroup, gio::ActionMap, gtk::Accessible, gtk::Buildable,
            gtk::ConstraintTarget, gtk::Native, gtk::Root, gtk::ShortcutManager;
}

impl Window {
    #[inline]
    #[must_use]
    pub fn new(app: &Application, settings: Settings) -> Self {
        let window: Self = Object::builder().property("application", app).build();

        let imp = window.imp();

        // Settings page has to be initialized before `load_and_setup_actions()`
        imp.init_settings_page(app.style_manager());

        window.load_and_setup_actions(&settings);
        window.setup_drag_and_drop();

        let _ = imp.settings.set(settings);

        window
    }

    #[inline]
    fn settings(&self) -> &Settings {
        self.imp().settings.get().expect(EXP_INIT)
    }

    /// Sets up functionality to accept external file drops
    ///
    /// # Panics
    /// The function panics if the file path is not valid UTF-8
    #[inline]
    fn setup_drag_and_drop(&self) {
        let drop_target =
            gtk::DropTarget::new(FileList::static_type(), DragAction::COPY | DragAction::MOVE);
        // TODO: Add visual feedback when the file is over the window
        drop_target.connect_drop(|_, value, _, _| {
            let files = (value.get::<FileList>().unwrap())
                .files()
                .iter()
                .map(|file| file.path().unwrap().to_str().unwrap().to_owned())
                .collect();
            (library_tx().send(LibraryRequest::QueueFromPaths(files))).expect(EXP_RX);
            true
        });
        self.add_controller(drop_target);
    }

    /// Saves the current window size to the settings
    ///
    /// # Errors
    /// The function errors if a `gio::Settings` value cannot be saved
    #[inline]
    pub fn save_window_size(&self) -> Result<(), glib::error::BoolError> {
        let settings = self.settings();
        settings.set_int("window-width", self.size(Orientation::Horizontal))?;
        settings.set_int("window-height", self.size(Orientation::Vertical))?;
        Ok(())
    }

    /// Saves all settings and the player state and prepares
    /// for shutdown, uninitializing various components
    ///
    /// # Errors
    /// The function errors if a `gio::Settings` value cannot be saved
    ///
    /// # Panics
    /// The function panics if either the library or player channel is closed
    pub fn save_and_uninit(&self) -> Result<(), glib::error::BoolError> {
        let _ = ui_tx().send(UpdateUI::Uninit);

        let imp = self.imp();
        let settings_page = &imp.settings_page;
        let remember_queue = settings_page.remembers_queue();
        let remember_time = settings_page.remembers_time();

        let library_tx = library_tx();
        (library_tx.send(LibraryRequest::CancelRebuild)).expect(EXP_RX);
        Library::run_task(library_tx, move || {
            LibraryConfig::create_config_dir();
            library_tx.send(LibraryRequest::Uninit).expect(EXP_RX);
            (player_tx().send(PlayerRequest::Uninit(remember_queue, remember_time))).expect(EXP_RX);
        });

        imp.artists_page.uninit();
        imp.albums_page.uninit();
        imp.songs_page.uninit();
        imp.queue_page.uninit();

        let settings = self.settings();
        settings.set_double("volume", settings_page.volume())?;
        settings.set_boolean("gapless", settings_page.gapless())?;
        settings.set_boolean("play-in-background", settings_page.play_in_background())?;
        settings.set_enum("startup-queue", *settings_page.startup_queue() as i32)?;
        settings.set_boolean("remember-time", remember_time)?;
        settings.set_boolean("adaptive-colors", settings_page.adaptive_colors())?;
        settings.set_enum("color-scheme", settings_page.color_scheme().cast_signed())?;
        settings.set_string("directories", &serialize_list(&settings_page.directories()))?;

        settings.set_string(
            "songs-sort",
            imp.songs_page.get_sort_config().ordering.get().to_str(),
        )?;
        settings.set_string(
            "albums-sort",
            imp.albums_page.get_sort_config().ordering.get().to_str(),
        )?;
        settings.set_string(
            "artists-sort",
            imp.artists_page.get_sort_config().ordering.get().to_str(),
        )?;

        settings.set_boolean("songs-shuffle", imp.songs_page.get_shuffle())?;
        settings.set_boolean("albums-shuffle", imp.albums_page.get_shuffle())?;
        settings.set_boolean("artists-shuffle", imp.artists_page.get_shuffle())?;

        Ok(())
    }

    /// Loads the application settings and sets up `gio` actions
    #[inline]
    fn load_and_setup_actions(&self, settings: &Settings) {
        let imp = self.imp();
        let settings_page = &imp.settings_page;

        // Slider callback `change_value` doesn't work for `set_value()`,
        // so the volume has to be set manually before setting the slider
        let volume = settings.double("volume");
        settings_page
            .imp()
            .handle_set_volume(gtk::ScrollType::Jump, volume);
        settings_page.set_volume(volume);
        settings_page.set_gapless(settings.boolean("gapless"));
        settings_page.set_play_in_background(settings.boolean("play-in-background"));
        settings_page.set_startup_queue(settings.enum_("startup-queue").into());
        settings_page.set_remember_time(settings.boolean("remember-time"));
        settings_page.set_adaptive_colors(settings.boolean("adaptive-colors"));
        settings_page.set_color_scheme(settings.enum_("color-scheme").cast_unsigned());

        let songs_sort = settings.string("songs-sort");
        let albums_sort = settings.string("albums-sort");
        let artists_sort = settings.string("artists-sort");
        imp.songs_page.set_sort_mode((&*songs_sort).into());
        imp.albums_page.set_sort_mode((&*albums_sort).into());
        imp.artists_page.set_sort_mode((&*artists_sort).into());

        imp.songs_page
            .set_shuffle(settings.boolean("songs-shuffle"));
        imp.albums_page
            .set_shuffle(settings.boolean("albums-shuffle"));
        imp.artists_page
            .set_shuffle(settings.boolean("artists-shuffle"));

        self.set_default_size(settings.int("window-width"), settings.int("window-height"));

        self.setup_actions(&songs_sort, &albums_sort, &artists_sort);
    }
}
