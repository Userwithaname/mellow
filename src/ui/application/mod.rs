use adw::{prelude::*, subclass::prelude::*};
use core::cell::RefCell;
use core::time::Duration;
use gtk::{gio, glib};
use std::panic;
use std::path::PathBuf;
use std::sync::mpsc;
#[cfg(feature = "event-timers")]
use std::time::Instant;
use std::{process, thread};

mod crash_handling;
mod imp;

use crate::excuses::EXP_INIT;
use crate::library::{Library, LibraryConfig, LibraryRequest, library_tx};
use crate::player::{Player, PlayerRequest, SongQueue};
use crate::shortcuts::Shortcuts;
use crate::ui::{Window, actions::Actions, ui_tx};
use crate::{about, music_dir, util::deserialize_list};
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
        #[cfg(feature = "event-timers")]
        let startup = Instant::now();

        let app: Self = glib::Object::builder()
            .property("application-id", about::APP_ID)
            .property("flags", gio::ApplicationFlags::HANDLES_OPEN)
            .build();
        glib::set_application_name(about::APP_NAME);
        glib::set_program_name(Some(about::APP_NAME.to_lowercase()));

        // Only runs once, because `init_channels` returns an error if already initialized
        if let Ok((player_rx, library_rx, ui_rx, mpris_rx)) = init_channels() {
            // Show an error and close the app entirely if any thread panics
            panic::set_hook(Box::new(|info| crash_handling::handle_crash(info, ui_tx())));

            // Starting the components in parallel with GTK (`init_components`)
            // results in faster launch times, but this requires moving them into
            // `connect_startup` which takes a reusable `Fn` closure. One way of
            // doing so is using a `RefCell<Option>` and `Option::take`.
            let settings = gio::Settings::new(about::APP_ID);
            let handles = Self::init_components(&settings, player_rx, library_rx);
            let args = RefCell::new(Some((settings, ui_rx, mpris_rx, handles)));
            let _ = gtk::init();

            #[cfg(feature = "gresources")]
            #[cfg(feature = "no-meson")]
            gio::resources_register_include!("mellow.gresource")
                .expect("Failed to register resources");

            #[cfg(feature = "gresources")]
            #[cfg(not(feature = "no-meson"))]
            gio::resources_register(
                &gio::Resource::load(about::resources_file())
                    .expect("Could not load resources file"),
            );

            app.connect_startup(move |app| {
                let Some((settings, ui_rx, mpris_rx, handles)) = args.take() else {
                    return; // This closure should not run multiple times
                };

                let window = Window::new(app, settings);
                glib::spawn_future_local({
                    let window = window.clone();
                    async move { window.imp().event_handler(ui_rx).await }
                });

                window.set_icon_name(Some(about::APP_ID));
                window.set_title(Some(about::APP_NAME));
                window.present();

                app.connect_activate(Self::show_window);
                app.connect_shutdown(Self::shutdown);

                app.setup_shortcuts();
                app.setup_actions(&window);

                let imp = app.imp();
                let _ = imp.window.set(window);
                imp.component_handles.set(Some(handles));

                glib::spawn_future_local(mpris::controller(mpris_rx));

                #[cfg(feature = "event-timers")]
                println!("\nWindow appeared {:?} since startup", startup.elapsed());
            });
        }

        app.connect_open(Self::open_files);

        app.run()
    }

    /// Starts the library and player threads and returns their handles
    #[inline]
    #[must_use]
    fn init_components(
        settings: &gio::Settings,
        player_rx: mpsc::Receiver<PlayerRequest>,
        library_rx: mpsc::Receiver<LibraryRequest>,
    ) -> [thread::JoinHandle<()>; 2] {
        let startup_queue = settings.enum_("startup-queue");
        let directories = settings.string("directories");
        [
            thread::Builder::new()
                .name("library".to_owned())
                .spawn(move || {
                    let mut library = Library::init(
                        LibraryConfig::new(match directories.as_str() {
                            // The value ":" means "first launch"
                            ":" => vec![PathBuf::from(music_dir())],
                            dirs => deserialize_list(dirs).iter().map(PathBuf::from).collect(),
                        }),
                        library_rx,
                    );
                    #[cfg(feature = "verbose-logs")]
                    println!("Library initialized");

                    SongQueue::init_queue(&library, startup_queue.into()).unwrap();
                    #[cfg(feature = "verbose-logs")]
                    println!("Queue was sent to the player");

                    library.build_global_tag_list();

                    // `STATE` does not need to be set here,
                    // because it defaults to `STATE_BUSY`
                    library.discover_files();
                    #[cfg(feature = "verbose-logs")]
                    println!("Files were checked");

                    library.request_handler().unwrap();
                })
                .unwrap(),
            thread::Builder::new()
                .name("player".to_owned())
                .spawn(move || Player::init(player_rx).controller().unwrap())
                .unwrap(),
        ]
    }

    /// Handles opening files from disk
    #[inline]
    fn open_files(&self, files: &[gio::File], _: &str) {
        let files = files.iter().map(|file| file.path().unwrap()).collect();
        let _ = library_tx().send(LibraryRequest::QueueFromPaths(files));
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
        let component_handles = imp.component_handles.take().unwrap().into_iter();
        let await_components = thread::spawn(move || {
            let results = component_handles.map(thread::JoinHandle::join);
            let _ = notify_done.send(());
            let _ = results.map(Result::unwrap); // Panic if any component has crashed
        });
        if rx.recv_timeout(timeout).is_err() {
            eprintln!("Exiting - component timeout was reached");
            process::exit(1);
        }
        await_components.join().expect("A component has crashed");
    }
}
