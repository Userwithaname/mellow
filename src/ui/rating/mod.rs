use adw::subclass::prelude::*;
use gtk::glib;

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

    /// Sets the rating and runs the `on_rating_set` closure
    #[inline]
    pub fn set_rating(&self, rating: SongRating) {
        self.imp().set_rating(rating);
    }

    /// Sets the rating without running the `on_rating_set` closure
    #[inline]
    pub fn set_rating_silent(&self, rating: SongRating) {
        let ui = self.imp();
        ui.rating.set(rating);
        ui.show_stars(rating.stars());
        ui.update_favorite_button(rating.is_favorite());
    }

    /// Connects a closure to run when a new rating is set
    #[inline]
    pub fn connect_rating_set<F>(&self, f: F)
    where
        F: Fn(SongRating) + Into<Box<F>> + 'static,
    {
        self.imp().on_rating_set.replace(Some(f.into()));
    }
}
