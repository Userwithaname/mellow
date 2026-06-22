use adw::{prelude::*, subclass::prelude::*};
use core::cell::{OnceCell, RefCell};
use core::sync::atomic::{AtomicBool, Ordering};
use glib::Properties;
use gtk::{gdk, glib};
use std::sync::Arc;

use crate::library::SharedAlbum;
use crate::ui::AlbumData;

#[derive(Properties, Default)]
#[properties(wrapper_type = super::AlbumObject)]
pub struct AlbumObject {
    #[property(name = "index", get, set, type = u32, member = index)]
    #[property(name = "album", get, set, type = String, member = album)]
    #[property(name = "artist", get, set, type = String, member = artist)]
    #[property(name = "artwork", get, set, type = Option<gdk::Paintable>, member = artwork)]
    #[property(name = "year", get, set, type = u32, member = year)]
    #[property(name = "rank", get, set, type = f64, member = rank)]
    #[property(name = "rating", get, set, type = f64, member = rating)]
    #[property(name = "played", get, set, type = f64, member = played)]
    #[property(name = "modified", get, set, type = u64, member = modified)]
    #[property(name = "added", get, set, type = u64, member = added)]
    #[property(name = "random", get, set, type = u64, member = random)]
    pub data: RefCell<AlbumData>,

    pub shared_album: OnceCell<SharedAlbum>,
    pub is_visible: Arc<AtomicBool>,
}

impl AlbumObject {
    #[inline]
    #[must_use]
    pub(super) fn shared_album(&self) -> &SharedAlbum {
        // SAFETY: Must be constructed using `AlbumObject::new()`
        unsafe { self.shared_album.get().unwrap_unchecked() }
    }
}

#[glib::object_subclass]
impl ObjectSubclass for AlbumObject {
    const NAME: &str = "MellowAlbumObject";
    type Type = super::AlbumObject;
}

#[glib::derived_properties]
impl ObjectImpl for AlbumObject {}

impl Drop for AlbumObject {
    fn drop(&mut self) {
        self.is_visible.store(false, Ordering::Release);
    }
}
