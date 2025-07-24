// TODO: make multiple files/modules with macros?

/// Has to be used inside `impl<...> RingBuffer<...>`.
macro_rules! impl_storage_common {
    () => {
        fn flags(&self) -> &AtomicU8 {
            &self.flags
        }

        unsafe fn slot_ptr(&self, pos: usize) -> *mut T {
            // SAFETY: See docstring.
            unsafe { self.data_ptr().add(self.collapse_position(pos)) }
        }
    };
}

/// Has to be used inside `impl<...> RingBuffer<...>`.
macro_rules! impl_storage_ptr_capacity {
    () => {
        fn capacity(&self) -> usize {
            self.capacity
        }

        fn data_ptr(&self) -> *mut T {
            self.data_ptr
        }
    };
}

/// Has to be used inside `impl<...> RingBuffer<...>`.
macro_rules! impl_index_calculation_double_size {
    () => {
        fn collapse_position(&self, pos: usize) -> usize {
            // Wraps a position from the range `0 .. 2 * capacity` to `0 .. capacity`.
            debug_assert!(pos == 0 || pos < 2 * self.capacity());
            if pos < self.capacity() {
                pos
            } else {
                pos - self.capacity()
            }
        }

        /// Increments a position by going `n` slots forward.
        fn increment(&self, pos: usize, n: usize) -> usize {
            debug_assert!(pos == 0 || pos < 2 * self.capacity());
            debug_assert!(n <= self.capacity());
            let threshold = 2 * self.capacity() - n;
            if pos < threshold {
                pos + n
            } else {
                pos - threshold
            }
        }

        /// Increments a position by going one slot forward.
        ///
        /// This might be more efficient than self.increment(..., 1).
        fn increment1(&self, pos: usize) -> usize {
            debug_assert_ne!(self.capacity(), 0);
            debug_assert!(pos < 2 * self.capacity());
            if pos < 2 * self.capacity() - 1 {
                pos + 1
            } else {
                0
            }
        }

        /// Returns the distance between two positions.
        fn distance(&self, a: usize, b: usize) -> usize {
            debug_assert!(a == 0 || a < 2 * self.capacity());
            debug_assert!(b == 0 || b < 2 * self.capacity());
            if a <= b {
                b - a
            } else {
                2 * self.capacity() - a + b
            }
        }
    };
}

//pub(crate) use index_calculation_double_size;
