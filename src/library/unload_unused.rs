use core::ops::Deref;

pub enum UsedBy {
    None = 0,
    Library = 1,
    SongQueue = 2,
}

/// Stores an `Option<T>`, which is automatically unloaded when no longer
/// marked as used by anything. Usages are managed manually. Note that marking
/// as used by the same variant multiple times is the same as marking it once.
pub struct UnloadUnused<T> {
    value: Option<T>,
    used_by: u8,
}

impl<T> Default for UnloadUnused<T> {
    #[inline]
    fn default() -> Self {
        UnloadUnused {
            value: None,
            used_by: UsedBy::None as u8,
        }
    }
}
impl<T> UnloadUnused<T> {
    /// Constructs a new `UnloadUnused` holding the `value`, with `used_by` specifying
    /// where it is used. `UsedBy::None` is also allowed, in which case the value will
    /// be unloaded the next time `mark_unused_by` is called, if no more uses were added.
    #[inline]
    pub const fn with_value(value: T, used_by: UsedBy) -> UnloadUnused<T> {
        UnloadUnused {
            value: Some(value),
            used_by: used_by as u8,
        }
    }
    /// Replaces the inner value with `value` and marks as used by `used_by`.
    /// If `UsedBy::None` is used, usages remain unchanged.
    #[inline]
    pub fn set_value(&mut self, value: Option<T>, used_by: UsedBy) {
        self.mark_used_by(used_by);
        self.value = value;
    }
    /// Marks `self` as used by `used_by`. `UsedBy::None` does nothing.
    #[inline]
    pub const fn mark_used_by(&mut self, used_by: UsedBy) {
        self.used_by |= used_by as u8;
    }
    /// Marks `self` as unused by `unused_by`. If there are no remaining
    /// uses, the inner `Option` is set to `None`.
    #[inline]
    pub fn mark_unused_by(&mut self, unused_by: UsedBy) {
        self.used_by &= !(unused_by as u8);
        if self.used_by == 0 {
            self.value = None;
        }
    }
    /// Sets the inner `Option` to `None`, but does not reset the usages
    #[inline]
    pub fn unload_value(&mut self) {
        self.value = None;
    }
}
impl<T> Deref for UnloadUnused<T> {
    type Target = Option<T>;

    #[inline]
    fn deref(&self) -> &Self::Target {
        &self.value
    }
}
