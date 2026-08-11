use std::ops::Deref;
use std::path::PathBuf;
use std::{fs, io};

/// Runs a closure for every file found within `dir` (recursive)
///
/// Adapted from the official Rust documentation:
/// <https://doc.rust-lang.org/std/fs/fn.read_dir.html#examples>
///
/// # Errors
/// - If `dir` is not a directory
/// - If a contained file or directory could not be read
#[inline]
pub fn visit_dirs<F: FnMut(PathBuf)>(dir: PathBuf, f: &mut F) -> io::Result<()> {
    for entry in fs::read_dir(dir)? {
        let path = entry?.path();
        match path.is_dir() {
            false => f(path),
            true => visit_dirs(path, f)?,
        }
    }
    Ok(())
}

/// Holds a shared `'static` reference to `T`
///
/// There is no way to drop the contained value, so this should _only_ be
/// used if the value is needed for the entire duration of the program
#[derive(Copy, Clone)]
pub struct Forever<T: 'static>(&'static T);
impl<T> Forever<T> {
    /// Constructs a new `Leaked` object, leaking `value` to a `'static` allocation
    #[inline]
    #[must_use]
    pub fn new(value: T) -> Forever<T> {
        Forever(Box::leak(Box::new(value)))
    }
    /// Returns a `'static` reference to the inner value
    #[inline]
    #[must_use]
    pub const fn static_ref(&self) -> &'static T {
        self.0
    }
}
impl<T: Default> Default for Forever<T> {
    #[inline]
    fn default() -> Self {
        Forever::new(T::default())
    }
}
impl<T> Deref for Forever<T> {
    type Target = T;

    #[inline]
    fn deref(&self) -> &'static Self::Target {
        self.0
    }
}
