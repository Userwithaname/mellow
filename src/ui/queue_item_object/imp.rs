use adw::{prelude::*, subclass::prelude::*};
use core::cell::{OnceCell, RefCell};
use core::sync::atomic::{AtomicBool, Ordering};
use glib::Properties;
use gtk::{gdk, glib};
use std::sync::Arc;

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
    }
}
