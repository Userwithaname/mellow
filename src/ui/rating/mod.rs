use adw::subclass::prelude::*;
use gtk::glib;

use crate::library::RatableAndTaggable;
use crate::library::song_rating::SongRating;

mod imp;

glib::wrapper! {
    pub struct Rating(ObjectSubclass<imp::Rating>)
        @extends gtk::Box, gtk::Widget,
        @implements
            gtk::Accessible, gtk::Actionable, gtk::Buildable, gtk::Orientable, gtk::ConstraintTarget;
}

impl Rating {
    /// Returns the current rating assigned to the widget
    #[inline]
    #[must_use]
    pub fn get_rating(&self) -> SongRating {
        self.imp().rating.get()
    }

    /// Sets the rating to the given `rating`
    #[inline]
    pub fn set_rating(&self, rating: SongRating) {
        self.imp().set_rating(rating);
    }

    /// Sets the item which the rating and tag changes will be forwarded to
    #[inline]
    pub fn set_item(&self, item: Box<dyn RatableAndTaggable>) {
        self.imp().set_item(item);
    }
}
