// storage: vec, array, vrb, dst
// indices & calculation: mop, bip (bip+vrb doesn't make sense)
// indices & padding: tight, padded
// chunks: vrb is a special case: only contiguous; bip could have both?
// owning p&c: vec, vrb, dst; non-owning: array, maybe dst?
// addressing: double size, pow2, single size, pow2-single; one_less (waste_one), unwrap; pow2; not_pow2
// size_type: usize, u32, u16, u8; (u64 and u128 probably don't make sense?)

macro_rules! storage_vec {
    (padded = $padded:ident, bip = $bip:ident, rb_doc = $rb_doc:expr) => {
        #[doc = $rb_doc]
        // TODO: manually derive Debug
        //#[derive(Debug)]
        pub struct RingBuffer<T> {
            head: def_padded!($padded, AtomicUsize),
            tail: def_padded!($padded, AtomicUsize),
            // TODO: measure whether CachePadded helps
            skip: def_only_bip!($bip, def_padded!($padded, AtomicUsize)),
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
                    head: init_padded!($padded, AtomicUsize::new(0)),
                    tail: init_padded!($padded, AtomicUsize::new(0)),
                    skip: init_only_bip!($bip, init_padded!($padded, AtomicUsize::new(NO_SKIP))),
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
    (padded = $padded:ident, bip = $bip:ident, rb_doc = $rb_doc:expr) => {
        use crate::atomic::*;
        use crate::cache_padded::CachePadded;
        use core::cell::UnsafeCell;
        use core::mem::MaybeUninit;

        #[doc = $rb_doc]
        // TODO: manually derive Debug
        //#[derive(Debug)]
        pub struct RingBuffer<T, const N: usize> {
            head: def_padded!($padded, AtomicUsize),
            tail: def_padded!($padded, AtomicUsize),
            // TODO: measure whether CachePadded helps
            skip: def_only_bip!($bip, def_padded!($padded, AtomicUsize)),
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
                    head: init_padded!($padded, AtomicUsize::new(0)),
                    tail: init_padded!($padded, AtomicUsize::new(0)),
                    skip: init_only_bip!($bip, init_padded!($padded, AtomicUsize::new(NO_SKIP))),
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

        impl<T, const N: usize> Default for RingBuffer<T, N> {
            fn default() -> Self {
                Self::new()
            }
        }
    };
}

macro_rules! def_padded {
    (yes, $ty:ty) => {
        CachePadded<$ty>
    };
    (no, $ty:ty) => {
        $ty
    };
}

macro_rules! def_only_bip {
    (yes, $init:ty) => {
        $init
    };
    (no, $init:ty) => {
        ()
    };
}

macro_rules! init_padded {
    (yes, $init:expr) => {
        CachePadded::new($init)
    };
    (no, $init:expr) => {
        $init
    };
}

macro_rules! init_only_bip {
    (yes, $init:expr) => {
        $init
    };
    (no, $init:expr) => {
        ()
    };
}

macro_rules! impl_drop_all_elements_helper {
    (
        self = $elf:ident,
        head = $head:ident,
        let_skip = ($($let_skip:tt)*),
        check_skip = ($($check_skip:tt)*),
        N = ($($N:ident)?)
    ) => {
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
            unsafe fn drop_all_elements(&mut $elf) {
                // These atomic variables are *not* used for synchronizing the threads
                // before destruction.  Relaxed ordering is sufficient here.
                let mut $head = $elf.head.load(Ordering::Relaxed);
                let tail = $elf.tail.load(Ordering::Relaxed);
                $($let_skip)*

                // Loop over all slots that hold a value and drop them.
                while $head != tail {
                    $($check_skip)*
                    // SAFETY: All slots between head and tail have been initialized.
                    unsafe { $elf.slot_ptr($head).drop_in_place() };
                    $head = $elf.increment1($head);
                }
            }
        }
    }
}

macro_rules! impl_drop_all_elements {
    (bip = no, N = ($($N:ident)?)) => {
        impl_drop_all_elements_helper! {
            self = self,
            head = head,
            let_skip = (),
            check_skip = (),
            N = ($($N)?)
        }
    };
    (bip = yes, N = ($($N:ident)?)) => {
        impl_drop_all_elements_helper! {
            self = self,
            head = head,
            let_skip = (
                let skip = self.skip.load(Ordering::Relaxed);
            ),
            check_skip = (
                if skip != NO_SKIP && head == skip {
                    head = self.increment(head, self.capacity() - self.collapse_position(skip));
                }
            ),
            N = ($($N)?)
        }
    };
}

macro_rules! impl_common {
    (N = ($($N:ident)?)) => {
        // SAFETY: RingBuffer is only mutated via Producer/Consumer (which are !Sync),
        // all other access can be shared.
        unsafe impl<T: Send$(, const $N: usize)?> Sync for RingBuffer<T$(, $N)?> {}

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
    (pow2 = no, N = ($($N:ident)?)) => {
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
    (pow2 = yes, N = ($($N:ident)?)) => {
        impl<T$(, const $N: usize)?> RingBuffer<T$(, $N)?> {
            // TODO:
        }
    };
}
