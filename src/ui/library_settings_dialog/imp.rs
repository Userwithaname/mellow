use std::fs;
use std::io::{BufReader, Read};
use std::path::PathBuf;
use std::time::Instant;

use adw::subclass::prelude::*;
use gio::prelude::*;
use gtk::CompositeTemplate;
use gtk::prelude::WidgetExt;
use gtk::{gio, glib};

use crate::library::{Library, Songs, library_tx};
use crate::ui::{UpdateUI, Window};
use crate::util::fs::{songs_file, thumbnails_dir, visit_dirs};

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
        Self::merge_library_data(|| {
            let songs_file = songs_file();
            [
                Library::deserialize_songs(&songs_file),
                Library::deserialize_songs(&[&songs_file, "~"].concat()),
            ]
            .concat()
        });
    }
    #[template_callback]
    pub fn handle_import_songs(&self) {
        let file_picker = gtk::FileDialog::builder().modal(true).build();
        let window = self.obj().root().and_downcast::<Window>().unwrap();
        file_picker.open(Some(&window), gio::Cancellable::NONE, move |file| {
            if let Ok(file) = file {
                Self::merge_library_data(move || {
                    Library::deserialize_songs(&file.path().unwrap().into_string().unwrap())
                });
            }
        });
    }
    #[template_callback]
    pub fn handle_export_songs(&self) {
        let file_picker = gtk::FileDialog::builder()
            .initial_name("songs")
            .modal(true)
            .build();
        let window = self.obj().root().and_downcast::<Window>().unwrap();
        file_picker.save(Some(&window), gio::Cancellable::NONE, move |file| {
            if let Ok(file) = file {
                Library::run_library_task(library_tx(), move |library| {
                    library.serialize_to_file(&file.path().unwrap().into_string().unwrap());
                });
            }
        });
    }
    fn merge_library_data<S: FnOnce() -> Songs + Send + 'static>(songs: S) {
        Library::run_library_task(library_tx(), |library| {
            library.cancel_library_build(Instant::now());
            let songs = songs();
            library.run_on_build_stopped(Box::new(|library| {
                let songs_len = songs.len();
                library.missing_songs.extend(songs);
                library.start_library_build(Instant::now());
                UpdateUI::Notification(
                    format!("Updated library data ({songs_len} candidates to resolve)"),
                    None,
                )
                .send();
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
