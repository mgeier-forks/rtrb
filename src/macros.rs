// TODO: make multiple files/modules with macros?

/// Has to be used inside `impl<...> RingBuffer<...>`.
macro_rules! impl_storage_ptr_capacity {
    () => {
    };
}

// storage: vec, array, vrb, dst
// indices & calculation: mop, bip (bip+vrb doesn't make sense)
// indices & padding: tight, padded
// chunks: vrb is a special case: only contiguous; bip could have both?
// owning p&c: vec, vrb, dst; non-owning: array, maybe dst?
// addressing: double size, pow2, single size, pow2-single; one_less, unwrap; pow2; not_pow2
// size_type: usize, u32, u16, u8; (u64 and u128 probably don't make sense?)

macro_rules! storage_vec {
    (padding = $padding:ident, partite = $partite:ident, rb_doc = $rb_doc:expr) => {
        #[doc = $rb_doc]
        // TODO: manually derive Debug
        //#[derive(Debug)]
        pub struct RingBuffer<T> {
            head: def_padded!($padding, AtomicUsize),
            tail: def_padded!($padding, AtomicUsize),
            // TODO: measure whether CachePadded helps
            skip: def_only_bip!($partite, def_padded!($padding, AtomicUsize)),
            flags: AtomicU8,
            data_ptr: *mut T,
            capacity: usize,
        }

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

            fn capacity(&self) -> usize {
                self.capacity
            }

            fn data_ptr(&self) -> *mut T {
                self.data_ptr
            }
        }

        impl<T> Drop for RingBuffer<T> {
            /// Drops all non-empty slots.
            fn drop(&mut self) {
                // SAFETY: this is called exactly once, no references to any elements exist anymore.
                unsafe { self.drop_all_elements() };

                // Finally, deallocate the buffer, but don't run any destructors.
                // SAFETY: data_ptr and capacity are still valid from the original initialization.
                unsafe { Vec::from_raw_parts(self.data_ptr, 0, self.capacity()) };
            }
        }
    };
}

macro_rules! storage_array {
    (padding = $padding:ident, partite = $partite:ident, rb_doc = $rb_doc:expr) => {
        use crate::atomic::*;
        use crate::cache_padded::CachePadded;
        use core::cell::UnsafeCell;
        use core::mem::MaybeUninit;

        #[doc = $rb_doc]
        // TODO: manually derive Debug
        //#[derive(Debug)]
        pub struct RingBuffer<T, const N: usize> {
            head: def_padded!($padding, AtomicUsize),
            tail: def_padded!($padding, AtomicUsize),
            // TODO: measure whether CachePadded helps
            skip: def_only_bip!($partite, def_padded!($padding, AtomicUsize)),
            flags: AtomicU8,
            /// The static array holding slots.
            ///
            /// This must be in an `UnsafeCell` because both producer and consumer
            /// have a (non-mutable) reference to the ring buffer and they use
            /// *interior mutability* to modify it.
            slots: UnsafeCell<[MaybeUninit<T>; N]>,
        }

        impl<T, const N: usize> RingBuffer<T, N> {
            pub const fn new() -> Self {
                const {
                    assert!(
                        // TODO: check if power of 2 is needed.
                        true, //Calc::from_u8(C).update_capacity(N) == N,
                        "`capacity` must be a power of two"
                    );
                }
                Self {
                    head: init_padded!($padding, AtomicUsize::new(0)),
                    tail: init_padded!($padding, AtomicUsize::new(0)),
                    skip: init_only_bip!(
                        $partite,
                        init_padded!($padding, AtomicUsize::new(NO_SKIP))
                    ),
                    flags: AtomicU8::new(0),
                    slots: UnsafeCell::new([const { MaybeUninit::uninit() }; N]),
                }
            }

            fn data_ptr(&self) -> *mut T {
                // TODO: what happens if N == 0?
                self.slots.get().cast()
            }

            fn capacity(&self) -> usize {
                N
            }
        }

        impl<T, const N: usize> Drop for RingBuffer<T, N> {
            fn drop(&mut self) {
                // SAFETY: this is called exactly once, no references to any elements exist anymore.
                unsafe { self.drop_all_elements() };
            }
        }
    };
}

macro_rules! def_padded {
    (tight, $ty:ty) => {
        $ty
    };
    (padded, $ty:ty) => {
        CachePadded<$ty>
    };
}

macro_rules! def_only_bip {
    (bip, $init:ty) => {
        $init
    };
    ($dummy1:ident, $dummy2:ty) => {
        ()
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
    ($dummy1:ident, $dummy2:expr) => {
        ()
    };
}

macro_rules! impl_partite {
    (partite = mop, N = $($N:ident)?) => {
        impl<T$(, const $N: usize)?> RingBuffer<T$(, $N)?> {
            /// Drop all elements that are still in the buffer.
            ///
            /// After this, head and tail indices are invalid.
            ///
            /// # Safety
            ///
            /// This can only be called in the `Drop` implementation of the ring buffer.
            ///
            /// The threads must have been synchronized before via `self.flags`.
            #[inline(never)]
            unsafe fn drop_all_elements(&mut self) {
                // These atomic variables are *not* used for synchronizing the threads
                // before destruction.  Relaxed ordering is sufficient here.
                let mut head = self.head.load(Ordering::Relaxed);
                let tail = self.tail.load(Ordering::Relaxed);

                // Loop over all slots that hold a value and drop them.
                while head != tail {
                    // SAFETY: All slots between head and tail have been initialized.
                    unsafe { self.slot_ptr(head).drop_in_place() };
                    head = self.increment1(head);
                }
            }
        }
    };
    (partite = bip, N = $($N:ident)?) => {};
}

macro_rules! impl_common {
    (N = $($N:ident)?) => {
        impl<T$(, const $N: usize)?> RingBuffer<T$(, $N)?> {
            unsafe fn slot_ptr(&self, pos: usize) -> *mut T {
                // SAFETY: See docstring.
                unsafe { self.data_ptr().add(self.collapse_position(pos)) }
            }
        }
    }
}

// TODO: another axis: unwrapped vs one_less
macro_rules! impl_calculation {
    (pow2 = false, N = $($N:ident)?) => {
        impl<T$(, const $N: usize)?> RingBuffer<T$(, $N)?> {
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
        }
    };
    (pow2 = true, N = $($N:ident)?,) => {
        impl<T$(, const $N: usize)?> RingBuffer<T$(, $N)?> {
        }
    };
}
