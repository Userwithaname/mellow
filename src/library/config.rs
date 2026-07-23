use std::fs;
use std::path::PathBuf;

use crate::config_dir;
use crate::library::{Library, LibraryRequest, library_tx};
use crate::ui::{UpdateUI, ui_tx};

pub const FILE_SUPPORT: &[&str] = &[
    "flac", "m4a", "mp3", "aac", "ac3", "wav",
    // TODO: Ensure all listed formats work
    // Untested:
    "ape", "mpc", "ogg",
];

#[derive(Clone)]
pub struct LibraryConfig {
    directories: Vec<PathBuf>,
}

impl LibraryConfig {
    /// Creates a new instance of `LibraryConfig` and assigns the provided `directories`
    #[inline]
    #[must_use]
    pub fn new(directories: Vec<PathBuf>) -> Self {
        let config = LibraryConfig { directories };
        let _ = ui_tx().send_blocking(UpdateUI::SetLibraryDirs(config.directories_string()));
        config
    }

    /// Returns the list of library directories
    #[inline]
    #[must_use]
    pub const fn directories(&self) -> &Vec<PathBuf> {
        &self.directories
    }

    /// Returns the list of library directories as `Vec<String>`
    ///
    /// # Panics
    /// Panics if `Path::to_str` conversion fails
    #[inline]
    #[must_use]
    pub fn directories_string(&self) -> Vec<String> {
        (self.directories.iter())
            .map(|dir| dir.to_str().unwrap().to_owned())
            .collect()
    }

    /// Replaces the configured directories with `dirs`
    pub fn set_libraries(&mut self, dirs: Vec<PathBuf>) {
        self.directories = dirs;
        self.directories.sort();
        println!(
            "Library directories updated\nLibraries: {:?}",
            self.directories
        );
        self.update_library();
    }

    /// Adds `dir` to the configured directories
    pub fn add_library(&mut self, dir: PathBuf) {
        // Using `!dir.iter().any(|_| true)` because `Path::is_empty` is unstable
        if self.directories.contains(&dir) || !dir.iter().any(|_| true) {
            // Needed to re-activate the directory settings UI
            let _ = ui_tx().send_blocking(UpdateUI::SetLibraryDirs(self.directories_string()));
            return;
        }
        let _ = ui_tx().send_blocking(UpdateUI::Progress(Some(0.0)));
        self.directories.push(dir);
        self.directories.sort();
        println!("Added a new library\nLibraries: {:?}", self.directories);
        self.update_library();
    }

    /// Removes the configured directory at `index`
    pub fn remove_library(&mut self, index: usize) {
        let ui_tx = ui_tx();
        let _ = ui_tx.send_blocking(UpdateUI::Progress(Some(0.0)));

        let removed_dir = self.directories.remove(index);

        let library_tx = library_tx();
        let _ = library_tx.send(LibraryRequest::RegisterUndoDirectory(removed_dir.clone()));
        Library::rebuild();

        println!("Removed a library\nLibraries: {:?}", self.directories);

        let _ = ui_tx.send_blocking(UpdateUI::Notification(
            format!("Removed a library directory: {removed_dir:?}"),
            Some(Box::new((
                "Undo",
                Box::new(move || {
                    let _ = library_tx.send(LibraryRequest::UndoRemovedDirectory(
                        removed_dir.clone(), //
                    ));
                }),
            ))),
        ));
        let _ = ui_tx.send_blocking(UpdateUI::SetLibraryDirs(self.directories_string()));
    }

    /// Requests a library rebuild and updates the directory list in the UI
    fn update_library(&self) {
        let _ = ui_tx().send_blocking(UpdateUI::SetLibraryDirs(self.directories_string()));
        Library::rebuild();
    }

    /// Creates the config directory if it does not exist yet
    ///
    /// # Panics
    /// Panics if directory creation fails
    #[inline]
    pub fn create_config_dir() {
        fs::create_dir_all(config_dir()).expect("Could not create the config directory");
    }
}
