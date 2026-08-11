use adw::{prelude::*, subclass::prelude::*};
use core::cell::Ref;
use gtk::{gdk, glib};

use crate::library::Songs;
use crate::ui::{LibrarySort, LibrarySortMode};

mod imp;

glib::wrapper! {
    pub struct SongsPage(ObjectSubclass<imp::SongsPage>)
        @extends adw::NavigationPage, gtk::Widget,
        @implements
            gtk::Accessible, gtk::Actionable, gtk::Buildable, gtk::Orientable, gtk::ConstraintTarget;
}

impl SongsPage {
    #[inline]
    pub fn load_songs(&self, songs: Songs) {
        glib::spawn_future_local(glib::clone!(
            #[weak(rename_to=songs_page)]
            self.imp(),
            async move { songs_page.load_songs(&songs).await }
        ));
    }

    pub fn focus_search(&self) {
        self.imp().search_entry.grab_focus();
    }

    #[inline]
    pub fn assign_artwork(&self, index: usize, artwork: Option<&gdk::Texture>) {
        self.imp().assign_artwork(index, artwork);
    }

    #[inline]
    pub fn set_sort_mode(&self, sort_mode: LibrarySortMode) {
        glib::spawn_future_local(glib::clone!(
            #[weak(rename_to=songs_page)]
            self.imp(),
            async move { songs_page.set_sort_mode(sort_mode).await }
        ));
    }
    #[inline]
    #[must_use]
    pub fn get_sort_config(&self) -> Ref<'_, LibrarySort> {
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

    #[inline]
    pub fn uninit(&self) {
        self.imp().uninit();
    }
}
