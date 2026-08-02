use std::cmp::Ordering;

pub trait CmpIsEqOr {
    fn is_eq_or(&self, ordering: Ordering) -> bool;
}

impl CmpIsEqOr for Ordering {
    /// Returns `true` if `self` matches `ordering`,
    /// or if `self` is `Ordering::Equal`
    ///
    /// # Example
    /// ```rust
    /// # use mellow::util::CmpIsEqOr;
    /// # use std::cmp::Ordering;
    /// #
    /// assert_eq!(10.cmp(&5).is_eq_or(Ordering::Greater), true);
    /// assert_eq!(5.cmp(&5).is_eq_or(Ordering::Greater), true);
    /// assert_eq!(0.cmp(&5).is_eq_or(Ordering::Greater), false);
    /// assert_eq!(10.cmp(&5).is_eq_or(Ordering::Less), false);
    /// assert_eq!(5.cmp(&5).is_eq_or(Ordering::Less), true);
    /// assert_eq!(0.cmp(&5).is_eq_or(Ordering::Less), true);
    /// ```
    #[inline]
    fn is_eq_or(&self, ordering: Ordering) -> bool {
        match self {
            Ordering::Equal => true,
            ord => ordering == *ord,
        }
    }
}
