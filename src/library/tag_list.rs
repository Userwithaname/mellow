use std::ops::Deref;
use std::sync::{RwLock, RwLockReadGuard, RwLockWriteGuard};

static GLOBAL_TAGS: RwLock<TagList> = RwLock::new(TagList::new());
/// Returns a guard for reading the global tags list
///
/// # Panics
/// Panics if the `RwLock` is poisoned
#[inline]
pub fn read_global_tags() -> RwLockReadGuard<'static, TagList> {
    GLOBAL_TAGS.read().unwrap()
}
/// Returns a guard for modifying the global tags list
///
/// # Panics
/// Panics if the `RwLock` is poisoned
#[inline]
pub(super) fn write_global_tags() -> RwLockWriteGuard<'static, TagList> {
    GLOBAL_TAGS.write().unwrap()
}

pub trait Taggable {
    fn get_tags(&self) -> Box<[String]>;
    fn add_tag(&self, tag: String);
    fn remove_tag(&self, tag: &str);
}

#[derive(Debug, Default)]
pub struct TagList(Vec<(String, usize)>);

impl TagList {
    /// Constructs an empty `TagList`
    #[inline]
    #[must_use]
    pub const fn new() -> TagList {
        TagList(Vec::new())
    }

    /// Returns a slice of all tags which are currently assigned,
    /// where the first tuple element is the tag name, and the second
    /// is the number of times that tag has been assigned
    #[inline]
    #[must_use]
    pub fn tags(&self) -> &[(String, usize)] {
        &self.0
    }
    /// Returns an iterator over all currently assigned tag names
    #[inline]
    pub fn tag_names(&self) -> impl Iterator<Item = &str> {
        self.0.iter().map(|(tag, _)| &**tag)
    }
    /// Returns an iterator over owned strings of all
    /// currently assigned tag names, by cloning them
    #[inline]
    pub fn tag_names_owned(&self) -> impl Iterator<Item = String> {
        self.0.iter().map(|(tag, _)| tag.clone())
    }
    /// Locates the given tag using binary search, and returns its index
    ///
    /// # Errors
    /// Returns an error if the tag was not found. The index from the `Err`
    /// variant can be used to insert the item at the proper position.
    #[inline]
    pub fn find(&self, tag: &str) -> Result<usize, usize> {
        self.0.binary_search_by(|(cur_tag, _)| (**cur_tag).cmp(tag))
    }
    /// Increases the reference count for the given `tag`,
    /// or adds it to the list if it is new
    #[inline]
    pub fn add(&mut self, tag: String) {
        match self.find(&tag) {
            // SAFETY: `Ok` variant of `TagList::find` is always within bounds
            Ok(index) => unsafe { self.0.get_unchecked_mut(index).1 += 1 },
            Err(index) => self.0.insert(index, (tag, 1)),
        }
    }
    /// Decreases the reference count for the given `tag`,
    /// or removes it from the list if it was the last one
    ///
    /// # Panics
    /// Panics in debug mode if the given tag is not in the list
    #[inline]
    pub fn remove(&mut self, tag: &str) {
        match self.find(tag) {
            Ok(index)
                // SAFETY: `Ok` variant of `TagList::find` is always within bounds
                if let count = unsafe { &mut self.0.get_unchecked_mut(index).1 }
                    && *count > 1 =>
            {
                *count -= 1;
            }
            Ok(index) => {
                self.0.remove(index);
            }
            Err(_) => cfg_select! {
                debug_assertions => panic!("Could not remove tag: \"{tag}\" (not found)"),
                _ => eprintln!("Could not remove tag: \"{tag}\" (not found)"),
            },
        }
    }

    /// Returns a mutable reference to the inner `Vec<(String, usize)>`
    ///
    /// # Warning
    /// Ensure the tags remain alphabetically sorted, or it will
    /// cause issues with the binary search
    #[inline]
    #[must_use]
    pub(super) const fn inner_mut(&mut self) -> &mut Vec<(String, usize)> {
        &mut self.0
    }
}

#[derive(Clone, Debug, Default)]
pub struct Tags(Vec<String>);

impl Tags {
    /// Returns a mutable reference to the inner `Vec<String>`
    ///
    /// # Warning
    /// Ensure the tags remain alphabetically sorted, or it will
    /// cause issues with the binary search
    #[inline]
    pub const fn get_mut(&mut self) -> &mut Vec<String> {
        &mut self.0
    }

    /// Locates the given tag using binary search, and returns its index
    ///
    /// # Errors
    /// Returns an error if the tag was not found. The index from the `Err`
    /// variant can be used to insert the item at the proper position.
    #[inline]
    pub fn find(&self, tag: &str) -> Result<usize, usize> {
        self.0.binary_search_by(|cur_tag| (**cur_tag).cmp(tag))
    }

    /// Adds `tag` to the list of tags if it is not currently present
    #[inline]
    pub fn add(&mut self, tag: String) {
        if let Err(index) = self.find(&tag) {
            self.0.insert(index, tag);
        }
    }
    /// Removes `tag` from the list of tags, if it is present
    #[inline]
    pub fn remove(&mut self, tag: &str) {
        if let Ok(index) = self.find(tag) {
            self.0.remove(index);
        }
    }
}

impl Deref for Tags {
    type Target = [String];

    #[inline]
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl From<Vec<String>> for Tags {
    #[inline]
    fn from(value: Vec<String>) -> Self {
        Tags(value)
    }
}
