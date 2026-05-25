use adw::subclass::prelude::*;
use core::mem::MaybeUninit;
use core::sync::atomic::{AtomicBool, Ordering};
use glib::Object;
use gtk::{gdk, glib};
use std::sync::Arc;

use crate::library::{Library, library_tx};
use crate::player::QueueItem;
use crate::ui::{UpdateUI, ui_tx};
use crate::util::format_duration_ms;

mod imp;

glib::wrapper! {
    /// # Safety
    /// Either construct using `QueueItemObject::new()`, or ensure
    /// that `….imp().queue_item` is initialized if constructing
    /// manually. Failing to do so will lead to undefined behavior.
    pub struct QueueItemObject(ObjectSubclass<imp::QueueItemObject>);
}

impl QueueItemObject {
    #[inline]
    #[must_use]
    pub fn new(index: u32, playing: bool, item: QueueItem) -> Self {
        let queue_object: QueueItemObject = Object::builder()
            .property("index", index)
            .property("playing", playing)
            .build();

        match &item {
            QueueItem::Song(song) => {
                let mut info = song.info();

                let mut guard = MaybeUninit::uninit();
                let song_info = info.get_basic(&mut guard);
                queue_object.set_title(song_info.title.clone());
                queue_object.set_subtitle(song_info.artist.clone());
                queue_object.set_suffix(format_duration_ms(song_info.duration_ms));

                if let Ok(thumbnail) = info.try_inspect_thumbnail()
                    && let Some(thumbnail) = thumbnail.as_ref()
                {
                    queue_object.set_artwork(thumbnail);
                }
            }
            QueueItem::Stopper(stopper) => {
                queue_object.set_title(stopper.display_name());
            }
        }

        let _ = queue_object.imp().queue_item.set(item);
        queue_object
    }

    /// Loads the artwork thumbnail in a background thread
    pub fn load_artwork(&self) {
        #[cfg(debug_assertions)]
        if self.artwork().is_some() {
            println!(
                "⚠️ Queue artwork already assigned - should this be checked in release builds as well?"
            );
            return;
        }
        let imp = self.imp();
        let index = self.index() as usize;
        let item = imp.queue_item().clone();
        let is_visible = Arc::clone(&imp.is_visible);
        is_visible.store(true, Ordering::Relaxed);
        Library::run_task(library_tx(), move || {
            if !is_visible.load(Ordering::Relaxed) {
                return;
            }
            let QueueItem::Song(song) = item else {
                return;
            };
            drop(song.info().load_thumbnail());
            let _ = ui_tx().send(UpdateUI::QueueSongLoaded { index, song });
        });
    }

    /// Returns a reference to the `QueueItem` associated with this object
    #[must_use]
    pub fn queue_item(&self) -> &QueueItem {
        self.imp().queue_item()
    }

    /// Returns `true` if the item is currently shown in the UI,
    /// otherwise it returns `false`
    #[must_use]
    pub fn is_visible(&self) -> &Arc<AtomicBool> {
        &self.imp().is_visible
    }
}

#[derive(Default)]
pub struct QueueItemData {
    index: u32,
    playing: bool,
    title: String,
    subtitle: String,
    suffix: String,
    artwork: Option<gdk::Texture>,
    selected: bool,
}
