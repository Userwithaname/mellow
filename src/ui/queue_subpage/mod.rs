use adw::{prelude::*, subclass::prelude::*};
use gtk::glib;
use std::cell::Ref;
use std::sync::Arc;

use crate::library::SharedSong;
use crate::player::{QueueItem, SharedStopper};

mod imp;

glib::wrapper! {
    pub struct QueueSubpage(ObjectSubclass<imp::QueueSubpage>)
        @extends adw::NavigationPage, gtk::Widget,
        @implements
            gtk::Accessible, gtk::Actionable, gtk::Buildable, gtk::Orientable, gtk::ConstraintTarget;
}

impl QueueSubpage {
    /// Updates the subpage contents using the info from `song`,
    /// and sets the `index` used for performing actions on the
    /// associated queue item
    pub fn show_song_info(&self, index: usize, song: SharedSong) {
        let song_page = self.imp();
        song_page.index.set(index);
        song.info().load_basic_and(|song_info| {
            song_page.song_title.set_label(&song_info.title);
            song_page.album_title.set_label(&song_info.album);
            song_page.artist_name.set_label(&song_info.artist);
        });

        let queue_item = QueueItem::from_song(&song);
        let album = song.get_album().as_ref().map(Arc::clone);
        let is_from_library = album.is_some();
        song_page.album.replace(album);
        song_page.rating.set_item(Box::new(song));
        song_page.rating.set_sensitive(is_from_library);
        song_page.go_to_album_button.set_sensitive(is_from_library);
        song_page.go_to_artist_button.set_sensitive(is_from_library);
        song_page.queue_item.replace(queue_item);

        self.show_song_elements(true);
    }

    /// Updates the subpage contents using the info from `stopper`,
    /// and sets the `index` used for performing actions on the
    /// associated queue item
    pub fn show_stopper_info(&self, index: usize, stopper: &SharedStopper) {
        let stopper_page = self.imp();
        stopper_page.index.set(index);
        stopper_page
            .queue_item
            .replace(QueueItem::from_stopper(stopper));

        let closes_player = stopper.should_close_player();
        stopper_page.stopper_closes_player.set_active(closes_player);
        stopper_page
            .song_title
            .set_label(SharedStopper::display_name_from_bool(closes_player));

        self.show_song_elements(false);
    }

    /// Refreshes the subpage UI using the same item as currently shown
    ///
    /// # Panics
    /// Panics if the page was not initialized
    pub fn refresh_ui(&self) {
        let ui = self.imp();
        let index = ui.index.get();
        match ui.queue_item.take() {
            QueueItem::Song(song) => self.show_song_info(index, song),
            QueueItem::Stopper(stopper) => self.show_stopper_info(index, &stopper),
        }
    }

    /// Returns the current queue subpage item index
    #[inline]
    #[must_use]
    pub fn index(&self) -> usize {
        self.imp().index.get()
    }
    /// Sets the subpage item index to the given value
    #[inline]
    pub fn set_index(&self, index: usize) {
        self.imp().index.set(index);
    }

    /// Returns a referecne to the currently assigned `QueueItem`
    #[inline]
    #[must_use]
    pub fn borrow_item(&self) -> Ref<'_, QueueItem> {
        self.imp().queue_item.borrow()
    }

    #[inline]
    fn show_song_elements(&self, is_song: bool) {
        let subpage = self.imp();
        subpage.album_title.set_visible(is_song);
        subpage.artist_name.set_visible(is_song);
        subpage.rating.set_visible(is_song);
        subpage.play_now_button.set_visible(is_song);
        subpage.stop_after_button.set_visible(is_song);
        subpage.stopper_closes_player.set_visible(!is_song);
        subpage.remove_song_button.set_visible(is_song);
        subpage.remove_stopper_button.set_visible(!is_song);
        subpage.go_to_album_button.set_visible(is_song);
        subpage.go_to_artist_button.set_visible(is_song);
    }

    /// Sets the 'Pause After' button text based on whether
    /// `stop_after` is `true` or `false`
    ///
    /// This is purely visual, and does not affect functionality
    #[inline]
    pub fn set_stop_after(&self, stop_after: bool) {
        let song_page = self.imp();
        song_page.stop_after.set(stop_after);
        song_page.stop_after_button.set_title(match stop_after {
            // TODO: Support translations
            true => "Do Not Pause After",
            false => "Pause After",
        });
    }
}
