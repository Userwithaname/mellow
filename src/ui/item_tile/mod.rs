use adw::{prelude::*, subclass::prelude::*};
use glib::Object;
use gtk::glib;

mod imp;

glib::wrapper! {
    pub struct ItemTile(ObjectSubclass<imp::ItemTile>)
        @extends gtk::Box, gtk::Widget,
        @implements
            gtk::Accessible, gtk::Actionable, gtk::Buildable, gtk::Orientable, gtk::ConstraintTarget;
}

impl Default for ItemTile {
    #[inline]
    fn default() -> Self {
        Object::builder().build()
    }
}

impl ItemTile {
    #[inline]
    #[must_use]
    pub fn builder() -> ItemTileBuilder {
        ItemTileBuilder {
            item_tile: Self::default(),
        }
    }

    #[inline]
    pub fn artwork_image(&self) -> &gtk::Picture {
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

pub struct ItemTileBuilder {
    item_tile: ItemTile,
}

impl ItemTileBuilder {
    #[inline]
    #[must_use]
    pub fn info(self, title: &str, subtitle: &str) -> Self {
        self.item_tile.set_info(title, subtitle);
        self
    }

    #[inline]
    #[must_use]
    pub fn show_artwork(self, show: bool) -> Self {
        self.item_tile.imp().image.set_visible(show);
        self
    }

    #[inline]
    #[must_use]
    pub fn image_css_classes(self, classes: &[&str]) -> Self {
        self.item_tile.imp().image.set_css_classes(classes);
        self
    }
    #[inline]
    #[must_use]
    pub fn title_css_classes(self, classes: &[&str]) -> Self {
        self.item_tile.imp().image.set_css_classes(classes);
        self
    }
    #[inline]
    #[must_use]
    pub fn subtitle_css_classes(self, classes: &[&str]) -> Self {
        self.item_tile.imp().image.set_css_classes(classes);
        self
    }

    #[inline]
    #[must_use]
    pub fn width_request(self, width: i32) -> Self {
        self.item_tile.set_width_request(width);
        self
    }
    #[inline]
    #[must_use]
    pub fn height_request(self, height: i32) -> Self {
        self.item_tile.set_height_request(height);
        self
    }

    #[inline]
    #[must_use]
    pub fn margin_top(self, margin: i32) -> Self {
        self.item_tile.set_margin_top(margin);
        self
    }
    #[inline]
    #[must_use]
    pub fn margin_bottom(self, margin: i32) -> Self {
        self.item_tile.set_margin_bottom(margin);
        self
    }

    #[inline]
    #[must_use]
    pub fn build(self) -> ItemTile {
        self.item_tile
    }
}
