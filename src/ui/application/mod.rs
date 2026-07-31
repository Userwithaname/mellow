use adw::{prelude::*, subclass::prelude::*};
use gtk::{gio, glib};
use std::cell::RefCell;
use std::panic::{self, PanicHookInfo};
use std::path::PathBuf;
use std::sync::mpsc;
use std::time::Duration;
use std::{process, thread};

mod imp;

use crate::excuses::{EXP_INIT, EXP_RX};
use crate::library::{Library, LibraryConfig, LibraryRequest, library_tx};
use crate::player::{Player, PlayerRequest, SongQueue};
use crate::shortcuts::Shortcuts;
use crate::ui::{UpdateUI, Window, actions::Actions, ui_tx};
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

        // Only runs once, because `init_channels` returns an error if already initialized
        if let Ok((player_rx, library_rx, ui_rx, mpris_rx)) = init_channels() {
            // Close the app entirely if a component thread panics
            panic::set_hook(Box::new(|info: &PanicHookInfo| {
                let location = match info.location() {
                    Some(location) => format!(
                        "{}@{}:{}",
                        location.file(),
                        location.line(),
                        location.column()
                    ),
                    None => "(unknown)".to_owned(),
                };
                let info = format!(
                    "Thread `{}` panicked at {location}:\n{}",
                    thread::current().name().unwrap_or_default(),
                    info.payload_as_str().unwrap_or_default()
                );
                eprintln!("{info}\n");

                // IDEA: Create a crash file on disk and attempt to correct the issue
                // on next launch (for example, if the `library` thread crashed, it
                // could perform a lengthy check to ensure the `songs` file is valid)

                if ui_tx().send_blocking(UpdateUI::CrashNotice(info)).is_err() {
                    process::exit(1);
                };
            }));

            // Starting the components in parallel with GTK (inside `init_componets`)
            // results in faster launch times, but this requires moving them into
            // `connect_startup` which takes a reusable `Fn` closure. One way of
            // doing so is using a `RefCell<Option>` and `Option::take`.
            let settings = app.init_components(player_rx, library_rx);
            let args = RefCell::new(Some((settings, ui_rx, mpris_rx)));
            app.connect_startup(move |app| {
                let Some((settings, ui_rx, mpris_rx)) = args.take() else {
                    return; // This closure should not run multiple times
                };
                Self::init_window(app, settings, ui_rx);
                glib::spawn_future_local(mpris::controller(mpris_rx));
            });
        }

        app.connect_open(Self::open_files);

        app.run()
    }

    /// Initializes the application window
    #[inline]
    fn init_window(&self, settings: gio::Settings, ui_rx: async_channel::Receiver<UpdateUI>) {
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
                            ":" => vec![PathBuf::from(music_dir())],
                            dirs => unescaped_split(dirs, ',')
                                .iter()
                                .map(PathBuf::from)
                                .collect(),
                        }),
                        library_rx,
                    );
                    #[cfg(feature = "startup-logs")]
                    println!("Library initialized");

                    SongQueue::init_queue(&library, startup_queue.into()).unwrap();
                    #[cfg(feature = "startup-logs")]
                    println!("Queue was sent to the player");

                    library.build_global_tag_list();

                    // `STATE` does not need to be set here,
                    // because it defaults to `STATE_BUSY`
                    library.discover_files();
                    #[cfg(feature = "startup-logs")]
                    println!("Files were checked");

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

        // NOTE: Uncomment the lines below to enable GResources

        // #[cfg(feature = "no-meson")]
        // gio::resources_register_include!("mellow.gresource").expect("Failed to register resources");

        // #[cfg(not(feature = "no-meson"))]
        // gio::resources_register(
        //     &gio::Resource::load(about::resources_file()).expect("Could not load resources file"),
        // );

        glib::set_application_name(about::app_name());
        glib::set_program_name(Some(about::app_name().to_lowercase()));

        settings
    }

    /// Returns the window associated with the `Application`
    ///
    /// # Panics
    /// Panics if the application `window` is uninitialized
    #[inline]
    #[must_use]
    pub fn window(&self) -> &Window {
        self.imp().window.get().expect(EXP_INIT)
    }

    /// Creates a new `Window` and presents it
    #[inline]
    fn create_window(&self, settings: gio::Settings, ui_rx: async_channel::Receiver<UpdateUI>) {
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
        let files = files.iter().map(|file| file.path().unwrap()).collect();
        (library_tx().send(LibraryRequest::QueueFromPaths(files))).expect(EXP_RX);
    }

    /// Shows the window if it is hidden
    #[inline]
    pub fn show_window(&self) {
        // FIX: Window does not get raised if already shown
        self.window().present();
    }

    /// Cleanly shuts down the application by saving the settings and state,
    /// and blocks until all other components stop running as well
    fn shutdown(&self) {
        let imp = self.imp();
        imp.window.get().unwrap().save_and_uninit().unwrap();

        // Wait until all components stop cleanly or the `timeout` is reached
        let timeout = Duration::from_secs(5);
        let (notify_done, rx) = mpsc::channel::<()>();
        let library_handle = imp.library_handle.take().unwrap();
        let player_handle = imp.player_handle.take().unwrap();
        let await_components = thread::spawn(move || {
            library_handle.join().unwrap();
            player_handle.join().unwrap();
            let _ = notify_done.send(());
        });
        if rx.recv_timeout(timeout).is_err() {
            eprintln!("Exiting - component timeout was reached");
            process::exit(1);
        }
        await_components.join().expect("A component has crashed");
    }
}
