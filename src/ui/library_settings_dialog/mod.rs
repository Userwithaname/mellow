use glib::Object;
use gtk::glib;

mod imp;

glib::wrapper! {
    pub struct LibrarySettingsDialog(ObjectSubclass<imp::LibrarySettingsDialog>)
        @extends adw::PreferencesDialog, adw::Dialog, gtk::Widget,
        @implements
            gtk::Accessible, gtk::Actionable, gtk::Buildable, gtk::Orientable, gtk::ConstraintTarget;
}

impl Default for LibrarySettingsDialog {
    fn default() -> Self {
        Object::builder().build()
    }
}
