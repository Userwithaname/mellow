use core::ops::Deref;

/// Holds a shared `'static` reference to `T`
///
/// There is no way to drop the contained value, so this should _only_ be
/// used if the value is needed for the entire duration of the program
pub struct Forever<T: 'static>(&'static T);
impl<T> Forever<T> {
    /// Constructs a new `Forever` object, leaking `value` to a `'static` allocation
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
