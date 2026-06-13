use adw::{prelude::*, subclass::prelude::*};
use core::cell::{OnceCell, RefCell};
use core::sync::atomic::{AtomicBool, Ordering};
use glib::Properties;
use gtk::{gdk, glib};
use std::sync::Arc;

use crate::library::SharedSong;
use crate::ui::SongData;

#[derive(Properties, Default)]
#[properties(wrapper_type = super::SongObject)]
pub struct SongObject {
    #[property(name = "index", get, set, type = u32, member = index)]
    #[property(name = "song", get, set, type = String, member = song)]
    #[property(name = "album", get, set, type = String, member = album)]
    #[property(name = "artist", get, set, type = String, member = artist)]
    #[property(name = "artwork", get, set, type = Option<gdk::Paintable>, member = artwork)]
    #[property(name = "year", get, set, type = u32, member = year)]
    #[property(name = "rank", get, set, type = f64, member = rank)]
    #[property(name = "rating", get, set, type = u8, member = rating)]
    #[property(name = "played", get, set, type = u64, member = played)]
    #[property(name = "modified", get, set, type = u64, member = modified)]
    #[property(name = "added", get, set, type = u64, member = added)]
    pub data: RefCell<SongData>,

    pub shared_song: OnceCell<SharedSong>,
    pub is_visible: Arc<AtomicBool>,
}
impl SongObject {
    #[inline]
    #[must_use]
    pub fn shared_song(&self) -> &SharedSong {
        // SAFETY: Must be constructed using `SongObject::new()`
        unsafe { self.shared_song.get().unwrap_unchecked() }
    }
}

#[glib::object_subclass]
impl ObjectSubclass for SongObject {
    const NAME: &str = "MellowSongObject";
    type Type = super::SongObject;
}

#[glib::derived_properties]
impl ObjectImpl for SongObject {}

impl Drop for SongObject {
    fn drop(&mut self) {
        self.is_visible.store(false, Ordering::Release);
    }
}
