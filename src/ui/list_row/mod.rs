use adw::{prelude::*, subclass::prelude::*};
use glib::Object;
use gtk::glib;

use crate::ui::gtk_ext::GtkPictureExt;

mod imp;

glib::wrapper! {
    pub struct ListRow(ObjectSubclass<imp::ListRow>)
        @extends adw::ActionRow, adw::PreferencesRow, gtk::ListBoxRow, gtk::Widget,
        @implements
            gtk::Accessible, gtk::Actionable, gtk::Buildable, gtk::Orientable, gtk::ConstraintTarget;
}

impl Default for ListRow {
    #[inline]
    fn default() -> Self {
        Object::builder().build()
    }
}

impl ListRow {
    #[inline]
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    #[inline]
    #[must_use]
    pub fn prefix_image(&self) -> &gtk::Picture {
        &self.imp().prefix_image
    }

    #[inline]
    pub fn set_suffix_label(&self, content: &str) {
        self.imp().suffix_label.set_label(content);
    }

    #[inline]
    pub fn set_image_margins(&self, margin: i32) {
        let row = self.imp();
        row.prefix_image.set_margin_top(margin);
        row.prefix_image.set_margin_bottom(margin);
    }

    #[inline]
    pub fn copy_from(&self, other: &Self) {
        self.set_title(&other.title());
        self.set_subtitle(&other.subtitle().unwrap_or_default());

        let row = self.imp();
        let other = other.imp();

        row.prefix_image
            .set_paintable(other.prefix_image.paintable().as_ref());
        row.suffix_label.set_label(&other.suffix_label.label());
    }

    #[inline]
    pub fn to_default(&self) {
        self.set_title("");
        self.set_subtitle("");

        let row = self.imp();
        row.prefix_image.set_blank();
        row.suffix_label.set_label("");
    }

    #[inline]
    pub fn set_selected(&self, selected: bool) {
        self.imp().set_selected(selected);
    }

    #[inline]
    pub fn add_binding(&self, binding: glib::Binding) {
        self.imp().bindings.borrow_mut().push(binding);
    }
    #[inline]
    pub fn add_bindings(&self, bindings: &[glib::Binding]) {
        self.imp().bindings.borrow_mut().extend_from_slice(bindings);
    }
    #[inline]
    pub fn reset_bindings(&self) {
        for binding in self.imp().bindings.borrow_mut().drain(..) {
            binding.unbind();
        }
    }
}
