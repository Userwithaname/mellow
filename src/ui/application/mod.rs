use adw::{prelude::*, subclass::prelude::*};
use gtk::{gio, glib};
use std::cell::RefCell;
use std::sync::mpsc;
use std::thread;
use tokio::sync::mpsc as tokio_mpsc;

mod imp;

use crate::excuses::{EXP_INIT, EXP_RX};
use crate::library::{Library, LibraryConfig, LibraryRequest, library_tx};
use crate::player::{Player, PlayerRequest, SongQueue};
use crate::shortcuts::Shortcuts;
use crate::ui::{UpdateUI, Window, actions};
use crate::{about, music_dir, util::unescaped_split};
use crate::{init_channels, mpris};

glib::wrapper! {
    pub struct Application(ObjectSubclass<imp::Application>)
        @extends gio::Application, adw::Application, gtk::Application,
        @implements gio::ActionGroup, gio::ActionMap;
}

impl Application {
    /// Initializes the player/library threads and the UI, then runs the application
    #[inline]
    pub fn run() -> glib::ExitCode {
        let app: Self = glib::Object::builder()
            .property("application-id", about::app_id())
            .property("flags", gio::ApplicationFlags::HANDLES_OPEN)
            .build();

        if let Ok((ui_rx, player_rx, library_rx, mpris_rx)) = init_channels() {
            // Starting the components in parallel with GTK initialization
            // results in faster launch times. Because `connect_startup` expects
            // a reusable `Fn` closure, `settings` and `ui_rx` are moved using
            // `RefCell<Option>` instead.
            let settings = app.init_components(player_rx, library_rx);
            let args = RefCell::new(Some((settings, ui_rx, mpris_rx)));
            app.connect_startup(move |app| {
                let (settings, ui_rx, mpris_rx) =
                    args.take().unwrap( /* closure should only run once */ );
                Self::init_window(app, settings, ui_rx);
                glib::spawn_future_local(mpris::controller(mpris_rx));
            });
        }

        app.connect_open(Self::open_files);

        app.run()
    }

    /// Initializes the application window
    #[inline]
    fn init_window(&self, settings: gio::Settings, ui_rx: tokio_mpsc::UnboundedReceiver<UpdateUI>) {
        self.create_window(settings, ui_rx);

        self.setup_actions();
        self.setup_shortcuts();

        self.connect_activate(Self::show_window);
        self.connect_shutdown(Self::shutdown);
    }

    /// Starts the player and library threads, calls `gtk::init`,
    /// registers resources, and returns the application settings
    #[inline]
    fn init_components(
        &self,
        player_rx: mpsc::Receiver<PlayerRequest>,
        library_rx: mpsc::Receiver<LibraryRequest>,
    ) -> gio::Settings {
        let settings = gio::Settings::new(about::app_id());
        let startup_queue = settings.enum_("startup-queue");
        let directories = settings.string("directories");

        let imp = self.imp();

        imp.library_handle.set(Some(
            thread::Builder::new()
                .name("library".to_owned())
                .spawn(move || {
                    let mut library = Library::init(
                        LibraryConfig::new(match directories.as_str() {
                            // The value ":" means "first launch"
                            ":" => vec![music_dir().clone()],
                            dirs => unescaped_split(dirs, ','),
                        }),
                        library_rx,
                    );
                    #[cfg(feature = "startup-logs")]
                    println!("Library initialized");

                    library.discover_files();
                    #[cfg(feature = "startup-logs")]
                    println!("Files were checked");

                    SongQueue::init_queue(&library, startup_queue.into()).unwrap();
                    #[cfg(feature = "startup-logs")]
                    println!("Queue was sent to the player");

                    library.request_handler().unwrap();
                })
                .unwrap(),
        ));

        imp.player_handle.set(Some(
            thread::Builder::new()
                .name("player".to_owned())
                .spawn(move || Player::init(player_rx).controller().unwrap())
                .unwrap(),
        ));

        let _ = gtk::init();

        #[cfg(feature = "no-meson")]
        gio::resources_register_include!("mellow.gresource").expect("Failed to register resources");

        #[cfg(not(feature = "no-meson"))]
        gio::resources_register(
            &gio::Resource::load(about::resources_file()).expect("Could not load resources file"),
        );

        glib::set_application_name(about::app_name());
        glib::set_program_name(Some(about::app_name().to_lowercase()));

        settings
    }

    /// Returns the window associated with the `Application`
    #[inline]
    #[must_use]
    fn window(&self) -> &Window {
        self.imp().window.get().expect(EXP_INIT)
    }

    /// Creates a new `Window` and presents it
    #[inline]
    fn create_window(
        &self,
        settings: gio::Settings,
        ui_rx: tokio_mpsc::UnboundedReceiver<UpdateUI>,
    ) {
        let window = Window::new(self, settings);
        #[cfg(feature = "startup-logs")]
        println!("Window created");

        glib::spawn_future_local({
            let window = window.clone();
            async move { window.imp().event_handler(ui_rx).await }
        });

        window.set_icon_name(Some(about::app_id()));
        window.set_title(Some(about::app_name()));
        window.present();
        #[cfg(feature = "startup-logs")]
        println!("Window presented");

        let _ = self.imp().window.set(window);
    }

    /// Handles opening files from disk
    #[inline]
    fn open_files(&self, files: &[gio::File], _: &str) {
        let files = files
            .iter()
            .map(|file| file.path().unwrap().to_str().unwrap().to_owned())
            .collect();
        (library_tx().send(LibraryRequest::QueueFromPaths(files))).expect(EXP_RX);
    }

    /// Shows the window if it is hidden
    #[inline]
    pub fn show_window(&self) {
        // FIX: Window does not get raised if already shown
        self.window().present();
    }

    /// Registers the application actions
    #[inline]
    fn setup_actions(&self) {
        self.add_action_entries([
            actions::app::show_window(self),
            actions::app::quit(self, self.window()),
        ]);
    }

    /// Cleanly shuts down the application by saving the settings and state,
    /// and blocks until all other components stop running as well
    fn shutdown(&self) {
        let imp = self.imp();
        imp.window.get().unwrap().save_and_uninit().unwrap();
        imp.library_handle.take().unwrap().join().unwrap();
        imp.player_handle.take().unwrap().join().unwrap();
    }
}
