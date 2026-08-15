use adw::{prelude::*, subclass::prelude::*};
use core::cell::{OnceCell, RefCell};
use core::sync::atomic::{AtomicBool, Ordering};
use glib::Properties;
use gtk::{gdk, glib};
use std::sync::Arc;

use crate::excuses::EXP_INIT;
use crate::player::QueueItem;
use crate::ui::QueueItemData;

#[derive(Properties, Default)]
#[properties(wrapper_type = super::QueueItemObject)]
pub struct QueueItemObject {
    #[property(name = "index", get, set, type = u32, member = index)]
    #[property(name = "playing", get, set, type = bool, member = playing)]
    #[property(name = "title", get, set, type = String, member = title)]
    #[property(name = "subtitle", get, set, type = String, member = subtitle)]
    #[property(name = "suffix", get, set, type = String, member = suffix)]
    #[property(name = "artwork", get, set, type = Option<gdk::Texture>, member = artwork)]
    #[property(name = "selected", get, set, type = bool, member = selected)]
    pub data: RefCell<QueueItemData>,

    pub queue_item: OnceCell<QueueItem>,
    pub is_visible: Arc<AtomicBool>,
}

impl QueueItemObject {
    #[inline]
    #[must_use]
    pub(super) fn queue_item(&self) -> &QueueItem {
        // SAFETY: Must be constructed using `QueueItemObject::new()`
        unsafe { self.queue_item.get().unwrap_unchecked() }
    }
}

#[glib::object_subclass]
impl ObjectSubclass for QueueItemObject {
    const NAME: &str = "MellowQueueItemObject";
    type Type = super::QueueItemObject;
}

#[glib::derived_properties]
impl ObjectImpl for QueueItemObject {}

impl Drop for QueueItemObject {
    fn drop(&mut self) {
        self.is_visible.store(false, Ordering::Release);
        // Reference count of 3 was chosen as the threshold for unused thumbnails based on testing.
        // The object is dropped before the UI is updated, so I count the following:
        // 1. The thumbnail loaded on `Song`
        // 2. Reference in `QueueItemData` (this object)
        // 3. Reference used by the UI row widget
        // Since the thumbnail is usually paired with an object (except in some cases temporarily),
        // the reference count for in-use thumbnails should be at least 4, even if unloaded from `Song`
        if (self.data.borrow().artwork.as_ref()).is_some_and(|a| a.ref_count() <= 3) {
            let song = self.queue_item.take().expect(EXP_INIT);
            let mut song_info = song.as_song().info();
            song_info.unload_thumbnail();
            song_info.unload_detailed(); // NOTE: Might require it's own reference count check
        }
    }
}
