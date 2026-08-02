use core::{mem, ptr};

pub trait ReorderVecExt {
    fn reorder(&mut self, index: usize, target: usize);
}
impl<T> ReorderVecExt for Vec<T> {
    /// Moves an element of `Vec<T>` from index `from` to `to`,
    /// preserving the order and shifting the elements in-between
    ///
    /// # Panics
    /// - If either `from` or `to` is out of bounds
    /// - If type `T` is zero-sized
    ///
    /// # Example
    /// ```rust
    /// # use mellow::util::ReorderVecExt;
    /// #
    /// let mut numbers = vec![1, 2, 3, 4, 5];
    ///
    /// numbers.reorder(1, 4);
    /// assert_eq!(numbers, [1, 3, 4, 5, 2]);
    ///
    /// numbers.reorder(4, 1);
    /// assert_eq!(numbers, [1, 2, 3, 4, 5]);
    ///
    /// let mut strings =  vec![
    ///     "a".to_owned(),
    ///     "b".to_owned(),
    ///     "much longer string to test if everything still works regardless".to_owned(),
    ///     "c".to_owned(),
    /// ];
    ///
    /// strings.reorder(2, 1);
    /// assert_eq!(
    ///     strings,
    ///     [
    ///         "a",
    ///         "much longer string to test if everything still works regardless",
    ///         "b",
    ///         "c",
    ///     ]
    /// );
    /// ```
    ///
    /// Reference counted types behave as expected:
    /// ```rust
    /// # use mellow::util::ReorderVecExt;
    /// use std::rc::Rc;
    ///
    /// let mut rcs = vec![Rc::new(1), Rc::new(2)];
    ///
    /// rcs.reorder(0, 1);
    /// assert_eq!(rcs, [2.into(), 1.into()]);
    /// assert_eq!(Rc::strong_count(&rcs[0]), 1);
    /// assert_eq!(Rc::strong_count(&rcs[1]), 1);
    /// ```
    fn reorder(&mut self, from: usize, to: usize) {
        assert!(mem::size_of::<T>() != 0, "Zero-sized types are unsupported");
        assert!(
            from < self.len() && to < self.len(),
            "Cannot reorder; index is out of range of {}:\n\tfrom:\t{from}\n\tto:\t{to}",
            self.len()
        );

        let ptr = self.as_mut_ptr();
        // SAFETY: Assert at the top ensures `from` is within bounds
        let old = unsafe { ptr::read(ptr.add(from)) };

        if from < to {
            // Copy everything after `from` up to and including `to` one to the left:
            // [++f---t++] => [++---tt++]

            // SAFETY: Because `from` and `to` are checked to be within bounds
            // and `from` < `to`, the following cannot exceed the allocation
            unsafe { ptr::copy(ptr.add(from + 1), ptr.add(from), to - from) };

            // Then overwrite the duplicate item using the original `from` value:
            // [++---tt++] => [++---tf++]
        } else {
            // Copy everything before `to` up to and including `from` one to the right:
            // [++t---f++] => [++tt---++]

            // SAFETY: Because `from` and `to` are checked to be within bounds
            // and `from` >= `to`, the following cannot exceed the allocation
            unsafe { ptr::copy(ptr.add(to), ptr.add(to + 1), from - to) };

            // Then overwrite the duplicate item using the original `from` value:
            // [++---tt++] => [++---tf++]
        }

        // Overwrite the position at `to` using the original value of `from`
        // SAFETY: Assert at the top ensures `to` is within bounds
        unsafe { ptr::write(ptr.add(to), old) };
    }
}
