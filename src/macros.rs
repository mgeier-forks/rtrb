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

// storage: vec, array, vrb, dst
// indices & calculation: mop, bip (bip+vrb doesn't make sense)
// indices & padding: tight, padded
// chunks: vrb is a special case: only contiguous; bip could have both?
// owning p&c: vec, vrb, dst; non-owning: array, maybe dst?
// addressing: double size, pow2, single size, pow2-single; ()
// size_type: usize, u32, u16, u8; (u64 and u128 probably don't make sense?)

macro_rules! storage_vec {
    ($padding:ident, $partite:ident) => {
        impl<T> RingBuffer<T> {
            #[allow(clippy::new_ret_no_self)]
            pub fn new(capacity: usize) -> (Producer<T>, Consumer<T>) {
                // TODO: update capacity if power of 2 is needed.
                //let capacity = Calc::from_u8(C).update_capacity(capacity);
                BoxedRingBuffer::new(Self {
                    head: init_padded!($padding, AtomicUsize::new(0)),
                    tail: init_padded!($padding, AtomicUsize::new(0)),
                    skip: init_only_bip!(
                        $partite,
                        init_padded!($padding, AtomicUsize::new(NO_SKIP))
                    ),
                    flags: AtomicU8::new(0),
                    data_ptr: ManuallyDrop::new(Vec::with_capacity(capacity)).as_mut_ptr(),
                    capacity,
                })
            }
        }
    };
}

macro_rules! init_padded {
    (tight, $init:expr) => {
        $init
    };
    (padded, $init:expr) => {
        CachePadded::new($init)
    };
}

macro_rules! init_only_bip {
    (bip, $init:expr) => {
        $init
    };
    ($whatever:tt) => {
        PhantomData
    };
}
