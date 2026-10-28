use std::path::PathBuf;

use crate::library::{Library, LibraryRequest};
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
        UpdateUI::SetLibraryDirs(config.directories_string()).send();
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
            UpdateUI::SetLibraryDirs(self.directories_string()).send();
            return;
        }
        UpdateUI::Progress(Some(0.0)).send();
        self.directories.push(dir);
        self.directories.sort();
        println!("Added a new library\nLibraries: {:?}", self.directories);
        self.update_library();
    }

    /// Removes the configured directory at `index`
    pub fn remove_library(&mut self, index: usize) {
        let ui_tx = ui_tx();
        UpdateUI::Progress(Some(0.0)).send_with(ui_tx);

        let removed_dir = self.directories.remove(index);

        LibraryRequest::RegisterUndoDirectory(removed_dir.clone()).send();
        Library::rebuild();

        println!("Removed a library\nLibraries: {:?}", self.directories);

        UpdateUI::Notification(
            format!("Removed a library directory: {removed_dir:?}"),
            Some(Box::new((
                "Undo",
                Box::new(move || LibraryRequest::UndoRemovedDirectory(removed_dir.clone()).send()),
            ))),
        )
        .send_with(ui_tx);
        UpdateUI::SetLibraryDirs(self.directories_string()).send_with(ui_tx);
    }

    /// Requests a library rebuild and updates the directory list in the UI
    fn update_library(&self) {
        UpdateUI::SetLibraryDirs(self.directories_string()).send();
        Library::rebuild();
    }
}
