use adw::prelude::*;
use gio::Cancellable;
use glib::subclass::prelude::*;
use gtk::{gio, glib};

use crate::excuses::EXP_RX;
use crate::library::{LibraryRequest, library_tx};
use crate::music_dir;
use crate::ui::{Application, Window};

#[inline]
pub fn add_library(window: Window) -> gio::ActionEntry<Application> {
    gio::ActionEntry::builder("add_library")
        .activate(move |_, _, _| {
            let filter = gtk::FileFilter::new();
            filter.add_mime_type("inode/directory");
            // FIX: Read-only should be selected by default
            let library_picker = gtk::FileDialog::builder()
                .modal(true)
                .default_filter(&filter)
                .accept_label("Add Library")
                .initial_folder(&gio::File::for_path(music_dir()))
                .build();

            let window = window.clone();
            library_picker.select_folder(Some(&window.clone()), Cancellable::NONE, move |dir| {
                if let Ok(dir) = dir {
                    library_tx()
                        .send(LibraryRequest::AddLibrary(dir.path().unwrap()))
                        .expect(EXP_RX);
                } else {
                    // Allow changing the library through the UI again if canceled,
                    // otherwise it will re-activate when the directory list is updated
                    window.imp().settings_page.allow_library_changes(true);
                }
            });
        })
        .build()
}

#[inline]
pub fn queue_from_disk(window: Window) -> gio::ActionEntry<Application> {
    gio::ActionEntry::builder("queue_from_disk")
        .activate(move |_, _, _| {
            let filter = gtk::FileFilter::new();
            filter.add_mime_type("audio/*");
            filter.add_mime_type("inode/directory");
            // FIX: Read-only should be selected by default
            let file_picker = gtk::FileDialog::builder()
                .modal(true)
                .default_filter(&filter)
                .accept_label("Play Now")
                .initial_folder(&gio::File::for_path(music_dir()))
                .build();

            file_picker.open_multiple(Some(&window), Cancellable::NONE, |dirs| {
                if let Ok(dirs) = dirs {
                    let mut paths = vec![];
                    let mut index = 0;
                    while let Some(path) = dirs.item(index) {
                        paths.push(path.downcast::<gio::File>().unwrap().path().unwrap());
                        index += 1;
                    }
                    library_tx()
                        .send(LibraryRequest::QueueFromPaths(paths))
                        .expect(EXP_RX);
                }
            });
        })
        .build()
}

#[inline]
pub fn show_window(app: &Application) -> gio::ActionEntry<Application> {
    gio::ActionEntry::builder("show_window")
        .activate(glib::clone!(
            #[weak]
            app,
            move |_, _, _| app.show_window()
        ))
        .build()
}

#[inline]
pub fn quit(app: &Application, window: &Window) -> gio::ActionEntry<Application> {
    gio::ActionEntry::builder("quit")
        .activate(glib::clone!(
            #[weak]
            window,
            #[weak]
            app,
            move |_, _, _| {
                window.close();
                app.quit();
            }
        ))
        .build()
}
