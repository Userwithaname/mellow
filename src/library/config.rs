use std::fs;

use crate::config_dir;
use crate::library::{LibraryRequest, library_tx};
use crate::ui::{UpdateUI, ui_tx};

pub const FILE_SUPPORT: &[&str] = &[
    "flac", "m4a", "mp3", "aac", "ac3", "wav",
    // TODO: Ensure all listed formats work
    // Untested:
    "ape", "mpc", "ogg",
];

#[derive(Clone)]
pub struct LibraryConfig {
    directories: Vec<String>,
}

impl LibraryConfig {
    /// Creates a new instance of `LibraryConfig` and assigns the provided `directories`
    #[inline]
    #[must_use]
    pub fn new(directories: Vec<String>) -> Self {
        let config = LibraryConfig { directories };
        let _ = ui_tx().send(UpdateUI::SetLibraryDirs(config.directories.clone()));
        config
    }

    /// Returns the list of library directories
    #[inline]
    #[must_use]
    pub const fn directories(&self) -> &Vec<String> {
        &self.directories
    }

    /// Replaces the configured directories with `dirs`
    pub fn set_libraries(&mut self, dirs: &[String]) {
        self.directories = dirs.into();
        self.directories.sort();
        println!(
            "Library directories updated\nLibraries: {:?}",
            self.directories
        );
        self.update_library();
    }

    /// Adds `dir` to the configured directories
    pub fn add_library(&mut self, dir: String) {
        if self.directories.contains(&dir) || dir.is_empty() {
            // Needed to re-activate the directory settings UI
            let _ = ui_tx().send(UpdateUI::SetLibraryDirs(self.directories.clone()));
            return;
        }
        let _ = ui_tx().send(UpdateUI::Progress(Some(0.0)));
        self.directories.push(dir);
        self.directories.sort();
        println!("Added a new library\nLibraries: {:?}", self.directories);
        self.update_library();
    }

    /// Removes the configured directory at `index`
    pub fn remove_library(&mut self, index: usize) {
        let ui_tx = ui_tx();
        let _ = ui_tx.send(UpdateUI::Progress(Some(0.0)));

        let removed_dir = self.directories.remove(index);

        let library_tx = library_tx();
        let _ = library_tx.send(LibraryRequest::RegisterUndoDirectory(removed_dir.clone()));
        let _ = library_tx.send(LibraryRequest::Rebuild);

        println!("Removed a library\nLibraries: {:?}", self.directories);

        let _ = ui_tx.send(UpdateUI::Notification(
            format!("Removed a library directory: {removed_dir}"),
            Some(Box::new((
                "Undo",
                Box::new(move || {
                    let _ = library_tx.send(LibraryRequest::UndoRemovedDirectory(
                        removed_dir.clone(), //
                    ));
                }),
            ))),
        ));
        let _ = ui_tx.send(UpdateUI::SetLibraryDirs(self.directories.clone()));
    }

    /// Requests a library rebuild and updates the directory list in the UI
    fn update_library(&self) {
        let _ = ui_tx().send(UpdateUI::SetLibraryDirs(self.directories.clone()));
        let _ = library_tx().send(LibraryRequest::Rebuild);
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
