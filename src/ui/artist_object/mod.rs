use adw::{prelude::*, subclass::prelude::*};
use core::cmp;
use glib::Object;
use gtk::{gdk, glib};

use crate::library::SharedArtist;
use crate::ui::SortConfig;

mod imp;

glib::wrapper! {
    /// # Safety
    /// Either construct using `ArtistObject::new()`, or ensure
    /// that `….imp().shared_artist` is initialized if constructing
    /// manually. Failing to do so will lead to undefined behavior.
    pub struct ArtistObject(ObjectSubclass<imp::ArtistObject>);
}

impl ArtistObject {
    #[inline]
    #[must_use]
    pub fn new(index: u32, artist: &str, albums: u64, shared_artist: SharedArtist) -> Self {
        let artist_object: ArtistObject = Object::builder()
            .property("index", index)
            .property("artist", artist)
            .property("albums", albums)
            .build();
        let _ = artist_object.imp().shared_artist.set(shared_artist);
        artist_object
    }

    #[inline]
    pub const fn load_artwork(&self) {
        // TODO: Decide what kind of image to show for library artists and construct it
        // Maybe 4 artworks composed in a grid with a circular cutout might look good
        // if self.artwork().is_some() {
        //     return;
        // }
        // let index = self.index() as usize;
        // Library::run_task(LIBRARY_TX.get().expect(EXP_INIT), move || {
        //     UI_TX
        //         .get()
        //         .expect(EXP_INIT)
        //         .send(UpdateUI::LibraryArtistLoaded(index))
        //         .expect(EXP_RX);
        // });
    }

    #[inline]
    pub fn unload_artwork(&self) {
        self.set_property("artwork", Option::<gdk::Texture>::None);
    }

    /// Returns the `SharedArtist` associated with this object
    #[inline]
    #[must_use]
    pub fn shared_artist(&self) -> &SharedArtist {
        self.imp().shared_artist()
    }

    /// Returns the ordering of `self` compared to `other`,
    /// based on the sort mode specified using `order_by`
    #[inline]
    #[must_use]
    pub fn order_cmp(&self, other: &Self, order_by: SortConfig<ArtistOrdering>) -> gtk::Ordering {
        let ord = match other.rank().total_cmp(&self.rank()) {
            cmp::Ordering::Equal => match order_by.ordering.get() {
                ArtistOrdering::Default => self.cmp_artist(other),
                ArtistOrdering::Added => self.cmp_added_newer(other),
                ArtistOrdering::Modified => self.cmp_modified_newer(other),
                ArtistOrdering::Random => self.cmp_random(other),
            },
            ordering => ordering,
        };
        if order_by.reversed.get() {
            return ord.reverse().into();
        }
        ord.into()
    }
    #[inline]
    #[must_use]
    fn cmp_artist(&self, other: &Self) -> cmp::Ordering {
        self.artist().cmp(&other.artist())
    }
    #[inline]
    #[must_use]
    fn cmp_added_newer(&self, other: &Self) -> cmp::Ordering {
        // NOTE: Comparing added time using the oldest
        // album's first song is not necessarily correct
        (other.added().cmp(&self.added())).then_with(|| self.index().cmp(&other.index()))
    }
    #[inline]
    #[must_use]
    fn cmp_modified_newer(&self, other: &Self) -> cmp::Ordering {
        (other.modified().cmp(&self.modified())).then_with(|| self.index().cmp(&other.index()))
    }
    #[inline]
    #[must_use]
    fn cmp_random(&self, other: &Self) -> cmp::Ordering {
        other.random().cmp(&self.random())
    }
}

#[derive(Default)]
pub struct ArtistData {
    index: u32,
    artist: String,
    albums: u64,
    artwork: Option<gdk::Paintable>,
    rank: f64,
    modified: u64,
    added: u64,
    random: u64,
}

#[derive(Clone, Copy)]
pub enum ArtistOrdering {
    // IDEA: Sort by average play count
    // IDEA: Sort by best average rating
    Default,
    Added,
    Modified,
    Random,
}

impl ArtistOrdering {
    #[inline]
    #[must_use]
    pub const fn to_str(self) -> &'static str {
        match self {
            ArtistOrdering::Default => "Default",
            ArtistOrdering::Added => "Added",
            ArtistOrdering::Modified => "Modified",
            ArtistOrdering::Random => "Random",
        }
    }
}
impl From<&str> for ArtistOrdering {
    #[inline]
    fn from(value: &str) -> Self {
        match value {
            "Default" => ArtistOrdering::Default,
            "Added" => ArtistOrdering::Added,
            "Modified" => ArtistOrdering::Modified,
            "Random" => ArtistOrdering::Random,
            _ => unimplemented!(),
        }
    }
}
