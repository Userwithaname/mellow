use adw::{prelude::*, subclass::prelude::*};
use gtk::glib;

use crate::library::Artists;
use crate::ui::{ArtistOrdering, SortConfig};

mod imp;

glib::wrapper! {
    pub struct ArtistsPage(ObjectSubclass<imp::ArtistsPage>)
        @extends adw::NavigationPage, gtk::Widget,
        @implements
            gtk::Accessible, gtk::Actionable, gtk::Buildable, gtk::Orientable, gtk::ConstraintTarget;
}

impl ArtistsPage {
    #[inline]
    pub fn load_artists(&self, artists: Artists) {
        glib::spawn_future_local(glib::clone!(
            #[weak(rename_to=artists_page)]
            self.imp(),
            async move { artists_page.load_artists(&artists).await }
        ));
    }

    pub fn focus_search(&self) {
        self.imp().search_entry.grab_focus();
    }

    #[inline]
    pub fn set_sort_mode(&self, sort_mode: ArtistOrdering) {
        glib::spawn_future_local(glib::clone!(
            #[weak(rename_to=artists_page)]
            self.imp(),
            async move { artists_page.set_sort_mode(sort_mode).await }
        ));
    }
    #[inline]
    #[must_use]
    pub fn get_sort_config(&self) -> &SortConfig<ArtistOrdering> {
        self.imp().get_sort_mode()
    }

    #[inline]
    pub fn set_shuffle(&self, shuffle: bool) {
        self.imp().set_shuffle(shuffle);
    }
    #[inline]
    #[must_use]
    pub fn get_shuffle(&self) -> bool {
        self.imp().get_shuffle()
    }
}
