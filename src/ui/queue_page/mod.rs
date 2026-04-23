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
    #[inline]
    pub fn init(&self, song_page: QueueSubpage) {
        let _ = self.imp().song_page.set(song_page);
    }

    #[inline]
    #[must_use]
    pub fn get_playing_index(&self) -> usize {
        self.imp().playing_index.get()
    }
    #[inline]
    pub fn set_playing_index(&self, index: usize) {
        self.imp().playing_index.set(index)
    }

    #[inline]
    #[must_use]
    pub fn get_shuffle(&self) -> bool {
        self.imp().shuffle_toggle.is_active()
    }
    pub fn update_shuffle(&self, shuffle: bool) {
        let ui = self.imp();
        ui.shuffle_toggle.set_icon_name(match shuffle {
            true => "media-playlist-shuffle-symbolic",
            false => "media-playlist-consecutive-symbolic",
        });
        ui.shuffle_toggle.set_active(shuffle);
        ui.next_scroll_pos.set(QueueScrollAction::ToPlaying);
    }

    #[inline]
    #[must_use]
    pub fn get_repeat(&self) -> bool {
        self.imp().repeat_toggle.is_active()
    }
    #[inline]
    pub fn update_repeat(&self, repeat: bool) {
        let ui = self.imp();
        ui.repeat_toggle.set_active(repeat);
    }

    #[inline]
    pub fn update_song_queue(&self, queue: Box<[QueueItem]>, index: usize) {
        let queue_page = self.imp();
        queue_page.replace_queue_items(queue);
        queue_page.update_song_queue(&queue_page.song_queue.borrow(), index);
    }
    #[inline]
    pub fn redraw_song_queue(&self) {
        let queue_page = self.imp();
        let (queue, index) = (
            &queue_page.song_queue.borrow(),
            queue_page.playing_index.get(),
        );
        self.imp().update_song_queue(queue, index);
    }
    #[inline]
    #[must_use]
    pub fn borrow_queue(&self) -> Ref<'_, Box<[QueueItem]>> {
        self.imp().song_queue.borrow()
    }

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
    #[default]
    Retain,
    Offset(i32),
    ToPlaying,
}
