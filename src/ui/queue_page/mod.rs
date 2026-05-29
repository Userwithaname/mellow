use adw::{prelude::*, subclass::prelude::*};
use gtk::{gdk, glib};
use std::cell::Ref;

use crate::player::QueueItem;
use crate::ui::QueueSubpage;

mod imp;

glib::wrapper! {
    pub struct QueuePage(ObjectSubclass<imp::QueuePage>)
        @extends adw::NavigationPage, gtk::Widget,
        @implements
            gtk::Accessible, gtk::Actionable, gtk::Buildable, gtk::Orientable, gtk::ConstraintTarget;
}

impl QueuePage {
    /// Initializes the `subpage`
    #[inline]
    pub fn init(&self, subpage: QueueSubpage) {
        let _ = self.imp().subpage.set(subpage);
    }

    /// Returns the current playing item index
    #[inline]
    #[must_use]
    pub fn get_playing_index(&self) -> usize {
        self.imp().playing_index.get()
    }
    /// Sets the current playing item index to `index`
    ///
    /// Note: `redraw_queue` must be called manually
    #[inline]
    pub fn set_playing_index(&self, index: usize) {
        self.imp().playing_index.set(index);
    }

    /// Returns the current shuffle mode setting (`true`/`false`)
    #[inline]
    #[must_use]
    pub fn get_shuffle(&self) -> bool {
        self.imp().shuffle_toggle.is_active()
    }
    /// Sets the shuffle button state to the value of `shuffle`,
    /// and forwards the shuffle mode to the player when handled
    pub fn update_shuffle(&self, shuffle: bool) {
        let ui = self.imp();
        ui.shuffle_toggle.set_icon_name(match shuffle {
            true => "media-playlist-shuffle-symbolic",
            false => "media-playlist-consecutive-symbolic",
        });
        ui.shuffle_toggle.set_active(shuffle);
        ui.next_scroll_pos.set(QueueScrollAction::ToPlaying);
    }

    /// Returns the current repeat mode setting (`true`/`false`)
    #[inline]
    #[must_use]
    pub fn get_repeat(&self) -> bool {
        self.imp().repeat_toggle.is_active()
    }
    /// Sets the repeat button state to the value of `repeat`,
    /// and forwards the repeat mode to the player when handled
    #[inline]
    pub fn update_repeat(&self, repeat: bool) {
        let ui = self.imp();
        ui.repeat_toggle.set_active(repeat);
    }

    /// Replaces the `queue` and `playing` index, then redraws the UI
    #[inline]
    pub fn update_queue(&self, queue: Box<[QueueItem]>, playing: usize) {
        let queue_page = self.imp();
        queue_page.set_queue_items(queue);
        queue_page.draw_queue(&queue_page.song_queue.borrow(), playing);
    }
    /// Redraws the song queue without changing it
    #[inline]
    pub fn redraw_queue(&self) {
        let queue_page = self.imp();
        let (queue, playing) = (
            &queue_page.song_queue.borrow(),
            queue_page.playing_index.get(),
        );
        queue_page.draw_queue(queue, playing);
    }

    /// Exits the selection mode if currently active
    #[inline]
    pub fn exit_selection(&self) {
        let queue_page = self.imp();
        if queue_page.selections.borrow().is_some() {
            queue_page.set_selection_mode(None);
        }
    }

    /// Returns a borrowed reference to the currently assigned song queue
    #[inline]
    #[must_use]
    pub fn borrow_queue(&self) -> Ref<'_, Box<[QueueItem]>> {
        self.imp().song_queue.borrow()
    }
    /// Returns the length of the currently assigned song queue
    #[inline]
    #[must_use]
    pub fn queue_length(&self) -> usize {
        self.imp().queue_length.get()
    }

    /// Assigns the `artwork` for the queue item at `index`
    ///
    /// The `index` is relative to the entire queue, rather than the displayed
    /// items. If the item at `index` is currently out of view, assigning the
    /// artwork is ignored.
    #[inline]
    pub fn assign_artwork(&self, index: usize, artwork: Option<&gdk::Texture>) {
        self.imp().assign_artwork(index, artwork);
    }

    /// Empties the list model, cancelling any pending background tasks during drop
    #[inline]
    pub fn uninit(&self) {
        self.imp().uninit();
    }
}

#[derive(Default, Debug, Clone, Copy)]
pub enum QueueScrollAction {
    /// The scroll position should remain the same after redraw
    #[default]
    Retain,
    /// The scroll position should be offset by N items
    Offset(i32),
    /// The scroll position should change to show the currently playing item
    ToPlaying,
}
