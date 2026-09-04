use adw::{prelude::*, subclass::prelude::*};
use glib::Object;
use gtk::glib;

mod imp;

glib::wrapper! {
    pub struct ItemRow(ObjectSubclass<imp::ItemRow>)
        @extends gtk::Box, gtk::Widget,
        @implements
            gtk::Accessible, gtk::Actionable, gtk::Buildable, gtk::Orientable, gtk::ConstraintTarget;
}

impl Default for ItemRow {
    #[inline]
    fn default() -> Self {
        Object::builder().build()
    }
}

impl ItemRow {
    #[inline]
    #[must_use]
    pub fn builder() -> ItemRowBuilder {
        ItemRowBuilder {
            item_row: Self::default(),
        }
    }

    #[inline]
    #[must_use]
    pub fn prefix_image(&self) -> &gtk::Picture {
        &self.imp().image
    }

    #[inline]
    pub fn set_info(&self, title: &str, subtitle: &str) {
        let ui = self.imp();
        ui.title.set_label(title);
        ui.subtitle.set_label(subtitle);
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

pub struct ItemRowBuilder {
    item_row: ItemRow,
}

impl ItemRowBuilder {
    #[inline]
    #[must_use]
    pub fn titles(self, album: &str, artist: &str) -> Self {
        self.item_row.set_info(album, artist);
        self
    }

    #[inline]
    #[must_use]
    pub fn image_css_classes(self, classes: &[&str]) -> Self {
        self.item_row.imp().image.set_css_classes(classes);
        self
    }

    #[inline]
    #[must_use]
    pub fn title_css_classes(self, classes: &[&str]) -> Self {
        self.item_row.imp().image.set_css_classes(classes);
        self
    }

    #[inline]
    #[must_use]
    pub fn subtitle_css_classes(self, classes: &[&str]) -> Self {
        self.item_row.imp().image.set_css_classes(classes);
        self
    }

    #[inline]
    #[must_use]
    pub fn build(self) -> ItemRow {
        self.item_row
    }
}
