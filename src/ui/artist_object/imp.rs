use adw::{prelude::*, subclass::prelude::*};
use core::cell::{OnceCell, RefCell};
use glib::Properties;
use gtk::{gdk, glib};

use crate::library::SharedArtist;
use crate::ui::ArtistData;

#[derive(Properties, Default)]
#[properties(wrapper_type = super::ArtistObject)]
pub struct ArtistObject {
    #[property(name = "index", get, set, type = u32, member = index)]
    #[property(name = "artist", get, set, type = String, member = artist)]
    #[property(name = "albums", get, set, type = u64, member = albums)]
    #[property(name = "artwork", get, set, type = Option<gdk::Paintable>, member = artwork)]
    #[property(name = "rank", get, set, type = f64, member = rank)]
    #[property(name = "stars", get, set, type = f64, member = stars)]
    #[property(name = "rating", get, set, type = f64, member = rating)]
    #[property(name = "played", get, set, type = f64, member = played)]
    #[property(name = "modified", get, set, type = u64, member = modified)]
    #[property(name = "added", get, set, type = u64, member = added)]
    #[property(name = "random", get, set, type = u64, member = random)]
    pub data: RefCell<ArtistData>,

    pub shared_artist: OnceCell<SharedArtist>,
}

impl ArtistObject {
    #[inline]
    #[must_use]
    pub(super) fn shared_artist(&self) -> &SharedArtist {
        // SAFETY: Must be constructed using `ArtistObject::new()`
        unsafe { self.shared_artist.get().unwrap_unchecked() }
    }
}

#[glib::object_subclass]
impl ObjectSubclass for ArtistObject {
    const NAME: &str = "MellowArtistObject";
    type Type = super::ArtistObject;
}

#[glib::derived_properties]
impl ObjectImpl for ArtistObject {}
