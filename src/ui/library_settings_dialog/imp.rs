use std::fs;
use std::io::{BufReader, Read};
use std::path::PathBuf;
use std::time::Instant;

use adw::subclass::prelude::*;
use gtk::CompositeTemplate;
use gtk::glib;

use crate::library::{Library, library_tx};
use crate::ui::UpdateUI;
use crate::util::visit_dirs;
use crate::{songs_file, thumbnails_dir};

#[derive(Default, CompositeTemplate)]
#[template(file = "library_settings_dialog.ui")]
pub struct LibrarySettingsDialog {
    #[template_child]
    enable_backups: TemplateChild<adw::SwitchRow>,
}

#[gtk::template_callbacks]
impl LibrarySettingsDialog {
    #[template_callback]
    pub fn handle_recover_backup(&self) {
        // IDEA: Also disable the button to avoid repeated interactions?
        Library::run_library_task(library_tx(), |library| {
            library.cancel_library_build(Instant::now());
            let old_songs = Library::deserialize_songs(&[&songs_file(), "~"].concat());
            UpdateUI::Notification(
                format!(
                    "Restoring library data from backup ({} songs)",
                    old_songs.len()
                ),
                None,
            )
            .send();
            library.run_on_build_stopped(Box::new(move |library| {
                library.missing_songs.extend(old_songs);
                library.start_library_build(Instant::now());
            }));
        });
    }
    #[template_callback]
    pub fn handle_reset_failed_thumbnails(&self) {
        let mut removed_count = 0;
        let _ = visit_dirs(PathBuf::from(thumbnails_dir()), &mut |path| {
            if !fs::File::open(&path).is_ok_and(|f| BufReader::new(f).read_exact(&mut [0]).is_ok())
                && fs::remove_file(path).is_ok()
            {
                removed_count += 1;
            }
        });
        UpdateUI::Notification(format!("Removed {removed_count} failed thumbnails"), None).send();
    }
    #[template_callback]
    pub fn handle_reset_all_thumbnails(&self) {
        let _ = fs::remove_dir_all(thumbnails_dir());
        Library::run_library_task(library_tx(), |library| {
            for song in library.songs() {
                song.unload_thumbnail();
            }
        });
        UpdateUI::Notification("Cleared all thumbnail cache".to_owned(), None).send();
    }
    #[template_callback]
    pub fn handle_reload_all_tags(&self) {
        // TODO: Ask the user for confirmation, as this may take a long time
        Library::run_library_task(library_tx(), |library| {
            for song in library.songs() {
                song.take_basic();
            }
            library.start_library_build(Instant::now());
        });
        UpdateUI::Notification("Reloading all song tags".to_owned(), None).send();
    }
}

#[glib::object_subclass]
impl ObjectSubclass for LibrarySettingsDialog {
    const NAME: &str = "MellowLibrarySettingsDialog";
    type Type = super::LibrarySettingsDialog;
    type ParentType = adw::PreferencesDialog;

    fn class_init(class: &mut Self::Class) {
        class.bind_template();
        class.bind_template_callbacks();
    }

    fn instance_init(obj: &glib::subclass::InitializingObject<Self>) {
        obj.init_template();
    }
}
impl ObjectImpl for LibrarySettingsDialog {
    fn constructed(&self) {
        let backup_songs_file = [&songs_file(), "~"].concat();
        self.enable_backups
            .set_active(fs::exists(&backup_songs_file).unwrap_or_default());
    }
    fn dispose(&self) {
        let backup_songs_file = [&songs_file(), "~"].concat();
        let _ = match self.enable_backups.is_active() {
            true if fs::exists(&backup_songs_file).unwrap_or_default() => return,
            true => fs::write(&backup_songs_file, ""),
            false => fs::remove_file(&backup_songs_file),
        };
    }
}

impl WidgetImpl for LibrarySettingsDialog {}
impl AdwDialogImpl for LibrarySettingsDialog {}
impl PreferencesDialogImpl for LibrarySettingsDialog {}
