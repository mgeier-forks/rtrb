// storage: vec, array, vrb, dst
// indices & calculation: mop, bip (bip+vrb doesn't make sense)
// indices & padding: tight, padded
// chunks: vrb is a special case: only contiguous; bip could have both?
// owning p&c: vec, vrb, dst; non-owning: array, maybe dst? "owned" module?
// addressing: double size, pow2, single size, pow2-single; one_less (waste_one), unwrap; pow2; not_pow2
// size_type: usize, u32, u16, u8; (u64 and u128 probably don't make sense?)

macro_rules! storage_vec {
    (padded = $padded:ident, bip = yes, rb_doc = $rb_doc:expr) => {
        storage_vec_helper!(padded = $padded, skip, rb_doc = $rb_doc);
    };
    (padded = $padded:ident, bip = no, rb_doc = $rb_doc:expr) => {
        storage_vec_helper!(padded = $padded, , rb_doc = $rb_doc);
    };
}

macro_rules! storage_vec_helper {
    (padded = $padded:ident, $($skip:ident)?, rb_doc = $rb_doc:expr) => {
        use $crate::atomic::*;
        use $crate::CachePadded;
        use alloc::vec::Vec;
        use core::mem::ManuallyDrop;

        #[doc = $rb_doc]
        // TODO: manually derive Debug
        //#[derive(Debug)]
        pub struct RingBuffer<T> {
            head: choice_ty!($padded, CachePadded<AtomicUsize>, AtomicUsize),
            tail: choice_ty!($padded, CachePadded<AtomicUsize>, AtomicUsize),
            $(
                // TODO: measure whether CachePadded helps
                $skip: choice_ty!($padded, CachePadded<AtomicUsize>, AtomicUsize),
            )?
            flags: AtomicU8,
            data_ptr: *mut T,
            capacity: usize,
        }

        // SAFETY: If T can be moved between threads, RingBuffer can as well.
        unsafe impl<T: Send> Send for RingBuffer<T> {}

        impl<T> RingBuffer<T> {
            // Private helper function.
            fn construct(capacity: usize) -> Self {
                Self {
                    head: choice!($padded,
                        CachePadded::new(AtomicUsize::new(0)),
                        AtomicUsize::new(0)),
                    tail: choice!($padded,
                        CachePadded::new(AtomicUsize::new(0)),
                        AtomicUsize::new(0)),
                    $(
                        $skip: choice!($padded,
                            CachePadded::new(AtomicUsize::new(capacity)),
                            AtomicUsize::new(capacity)),
                    )?
                    flags: AtomicU8::new(0),
                    data_ptr: ManuallyDrop::new(Vec::with_capacity(capacity)).as_mut_ptr(),
                    capacity,
                }
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
    (arc = $arc:ident, padded = $padded:ident, bip = yes, rb_doc = $rb_doc:expr) => {
        storage_array_helper!(arc = $arc, padded = $padded, skip, rb_doc = $rb_doc);
    };
    (arc = $arc:ident, padded = $padded:ident, bip = no, rb_doc = $rb_doc:expr) => {
        storage_array_helper!(arc = $arc, padded = $padded, , rb_doc = $rb_doc);
    };
}

macro_rules! storage_array_helper {
    (arc = $arc:ident, padded = $padded:ident, $($skip:ident)?, rb_doc = $rb_doc:expr) => {
        use $crate::atomic::*;
        use $crate::cache_padded::CachePadded;
        use core::cell::UnsafeCell;

        #[doc = $rb_doc]
        // TODO: manually derive Debug
        //#[derive(Debug)]
        pub struct RingBuffer<T, const N: usize> {
            head: choice_ty!($padded, CachePadded<AtomicUsize>, AtomicUsize),
            tail: choice_ty!($padded, CachePadded<AtomicUsize>, AtomicUsize),
            $(
                // TODO: measure whether CachePadded helps
                $skip: choice_ty!($padded, CachePadded<AtomicUsize>, AtomicUsize),
            )?
            flags: AtomicU8,
            /// The static array holding slots.
            ///
            /// This must be in an `UnsafeCell` because both producer and consumer
            /// have a (non-mutable) reference to the ring buffer and they use
            /// *interior mutability* to modify it.
            slots: UnsafeCell<[MaybeUninit<T>; N]>,
        }

        // SAFETY: If T can be moved between threads, RingBuffer can as well.
        unsafe impl<T: Send, const N: usize> Send for RingBuffer<T, N> {}

        impl<T, const N: usize> RingBuffer<T, N> {
            // Private helper function.
            const fn construct() -> Self {
                Self {
                    head: choice!($padded,
                        CachePadded::new(AtomicUsize::new(0)),
                        AtomicUsize::new(0)),
                    tail: choice!($padded,
                        CachePadded::new(AtomicUsize::new(0)),
                        AtomicUsize::new(0)),
                    $(
                        $skip: choice!($padded,
                            CachePadded::new(AtomicUsize::new(N)),
                            AtomicUsize::new(N)),
                    )?
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

        storage_array_impl_default_for_ring_buffer!(arc = $arc);
    };
}

macro_rules! storage_array_impl_default_for_ring_buffer {
    (arc = yes) => {};
    (arc = no) => {
        impl<T, const N: usize> Default for RingBuffer<T, N> {
            fn default() -> Self {
                Self::new()
            }
        }
    };
}

macro_rules! choice {
    (yes, $yes:expr, $no:expr) => {
        $yes
    };
    (no, $yes:expr, $no:expr) => {
        $no
    };
}

macro_rules! choice_ty {
    (yes, $yes:ty, $no:ty) => {
        $yes
    };
    (no, $yes:ty, $no:ty) => {
        $no
    };
}

// TODO: rename
macro_rules! impl_everything_eventually {
    (
        arc = $arc:ident,
        array = $array:ident,
        bip = $bip:ident,
        contiguous = $contiguous:ident,
        pow2 = $pow2:ident,
        module = $module:literal,
    ) => {
        check_bip_contiguous!($bip, $contiguous);

        use core::cell::Cell;
        use core::mem::MaybeUninit;

        // TODO: move definition here?
        #[allow(unused_imports)]
        use $crate::diy::IS_ABANDONED;
        #[allow(unused_imports)]
        use $crate::diy::{HAS_CONSUMER, HAS_PRODUCER};

        // TODO: move error type to top level?
        pub use $crate::chunks::ChunkError;

        /// Error type for [`Consumer::peek()`].
        #[doc(inline)]
        pub use $crate::PeekError;
        /// Error type for [`Consumer::pop()`].
        #[doc(inline)]
        pub use $crate::PopError;
        /// Error type for [`Producer::push()`].
        #[doc(inline)]
        pub use $crate::PushError;

        /// Extension trait providing a [`copy_to_uninit()`](CopyToUninit::copy_to_uninit)
        /// method on built-in slices.
        ///
        /// This can be used to safely copy data to the
        #[doc = choice!($contiguous,
            "slice returned from [`WriteChunkUninit::as_mut_slice()`].",
            "slices returned from [`WriteChunkUninit::as_mut_slices()`].")]
        ///
        /// To use this, the trait has to be brought into scope, e.g. with:
        ///
        /// ```
        #[doc = concat!("use ", $module, "::CopyToUninit as _;")]
        /// ```
        #[doc(inline)]
        pub use $crate::CopyToUninit;

        // SAFETY: RingBuffer is only mutated (using *interior mutablility*)
        // via Producer/Consumer (which are !Sync), all other access can be shared.
        unsafe_impl!(Sync, for = ty!(RingBuffer, array = $array), array = $array, where T: Send);

        impl_!(RingBuffer, array = $array, {
            fn_ring_buffer_new!(arc = $arc, array = $array, pow2 = $pow2, module = $module);
            fn_ring_buffer_producer!(arc = $arc, array = $array);
            fn_ring_buffer_consumer!(arc = $arc, array = $array);

            fn_ring_buffer_drop_all_elements!(bip = $bip);
            fn_ring_buffer_update_capacity!(pow2 = $pow2);
            fn_ring_buffer_collapse_position!(pow2 = $pow2);
            fn_ring_buffer_slot_ptr!();
            fn_ring_buffer_increment!(pow2 = $pow2);
            fn_ring_buffer_increment1!(pow2 = $pow2);
            fn_ring_buffer_distance!(pow2 = $pow2);
        });

        struct_arc_ring_buffer!(arc = $arc, array = $array);

        struct_producer!(arc = $arc, array = $array);

        impl_!(Producer, arc = $arc, array = $array, {
            fn_producer_push!(arc = $arc, array = $array, module = $module);
            fn_producer_write_chunk_uninit!(array = $array, bip = $bip);
            fn_producer_write_chunk!(array = $array, contiguous = $contiguous);
            // TODO: documentation specific to bip:
            fn_producer_slots!(arc = $arc, array = $array, module = $module);
            fn_producer_slots_contiguousX!(bip = $bip);
            fn_producer_is_full!(arc = $arc, array = $array, module = $module);
            fn_pc_capacity!(arc = $arc, array = $array, module = $module);
            fn_producer_is_abandoned!(arc = $arc, array = $array, module = $module);

            fn_producer_next_tail!();
        });

        struct_consumer!(arc = $arc, array = $array);

        impl_!(Consumer, arc = $arc, array = $array, {
            fn_consumer_pop!(arc = $arc, array = $array, module = $module);
            fn_consumer_peek!(arc = $arc, array = $array, module = $module);
            fn_consumer_read_chunk!(array = $array, bip = $bip);
            // TODO: documentation specific to bip:
            fn_consumer_slots!(arc = $arc, array = $array, bip = $bip, module = $module);
            fn_consumer_slots_contiguousX!(bip = $bip);
            fn_consumer_is_empty!(arc = $arc, array = $array, module = $module);
            fn_pc_capacity!(arc = $arc, array = $array, module = $module);
            fn_consumer_is_abandoned!(arc = $arc, array = $array, module = $module);

            fn_consumer_next_head!(bip = $bip);
        });

        struct_write_chunk_uninit!(arc = $arc, array = $array, contiguous = $contiguous);
        struct_write_chunk!(array = $array);
        struct_read_chunk!(arc = $arc, array = $array, contiguous = $contiguous);

        impl_send_for_chunks!(array = $array, module = $module);

        impl_!(WriteChunkUninit<'_>, array = $array, {
            fn_write_chunk_uninit_as_mut_sliceX!(contiguous = $contiguous);
            fn_write_chunk_uninit_commit_all!();
            fn_write_chunk_uninit_commit!();
            fn_write_chunk_uninit_fill_from_iter!(arc = $arc, array = $array, contiguous = $contiguous, module = $module);
            fn_X_chunk_X_len_and_is_empty!(contiguous = $contiguous);

            fn_write_chunk_uninit_drop_suffix!(contiguous = $contiguous);
            fn_write_chunk_uninit_commit_unchecked!(bip = $bip);
        });

        impl_!(WriteChunk<'_>, array = $array, {
            fn_write_chunk_as_mut_sliceX!(contiguous = $contiguous);
            fn_write_chunk_commit_all!();
            fn_write_chunk_commit!();
            fn_write_chunk_len_and_is_empty!();
        });

        impl_!(ReadChunk<'_>, array = $array, {
            fn_read_chunk_as_sliceX!(contiguous = $contiguous);
            fn_read_chunk_as_mut_sliceX!(contiguous = $contiguous);
            fn_read_chunk_commit_all!();
            fn_read_chunk_commit!(arc = $arc, array = $array, module = $module);
            fn_X_chunk_X_len_and_is_empty!(contiguous = $contiguous);

            fn_read_chunk_uninit_commit_unchecked!(contiguous = $contiguous);
        });
    };
}

macro_rules! check_bip_contiguous {
    (yes, no) => {
        compile_error!("`bip = yes` requires `contiguous = yes`");
    };
    (no, yes) => {
        // This is used for "vrb".
    };
    (yes, yes) => {};
    (no, no) => {};
}

macro_rules! docstring {
    ($(#[doc = $line:expr])*) => {
        concat!($($line, "\n"),*)
    };
}

macro_rules! doctest_import {
    ($module:literal, $items:literal$(, $prefix:literal)?) => {
        concat!($($prefix, )?"use ", $module, "::", $items, ";")
    };
}

macro_rules! doctest_create_ring_buffer {
    (arc = yes, array = yes, capacity = $capacity:literal$(, $prefix:literal)?) => {
        concat!(
            $($prefix, )?
            "let (mut p, mut c) = RingBuffer::<_, ",
            $capacity,
            ">::new();"
        )
    };
    (arc = yes, array = no, capacity = $capacity:literal$(, $prefix:literal)?) => {
        concat!($($prefix, )?"let (mut p, mut c) = RingBuffer::new(", $capacity, ");")
    };
    (arc = no, array = yes, capacity = $capacity:literal$(, $prefix:literal)?) => {
        concat!(
            $($prefix, )?
            "let rb = RingBuffer::<_, ",
            $capacity,
            ">::new();\n",
            $($prefix, )?
            "let mut p = rb.producer().unwrap();\n",
            $($prefix, )?
            "let mut c = rb.consumer().unwrap();",
        )
    };
    (arc = no, array = no, capacity = $capacity:literal$(, $prefix:literal)?) => {
        concat!(
            $($prefix, )?
            "let rb = RingBuffer::new(",
            $capacity,
            ");\n",
            $($prefix, )?
            "let mut p = rb.producer().unwrap();\n",
            $($prefix, )?
            "let mut c = rb.consumer().unwrap();",
        )
    };
}

macro_rules! doctest_ty {
    ($name:ident, $ty:ty, $N:literal, array = yes) => {
        concat!(stringify!($name), "<", stringify!($ty), ", ", $N, ">")
    };
    ($name:ident, $ty:ty, $N:literal, array = no) => {
        concat!(stringify!($name), "<", stringify!($ty), ">")
    };
}

macro_rules! struct_ {
    ($(#[$attr:meta])* $name:ident, array = yes, params = ($($params:tt)+), fields = $fields:tt $($end:tt)?) => {
        $(#[$attr])* pub struct $name<$($params)+, const N: usize> $fields $($end)?
    };
    ($(#[$attr:meta])* $name:ident, array = no, params = ($($params:tt)+), fields = $fields:tt $($end:tt)?) => {
        $(#[$attr])* pub struct $name<$($params)+> $fields $($end)?
    };
}

// $body can start with a "where" clause, if needed.
macro_rules! impl_ {
    ($name:ident<'_>, $(trait = $trait:ty,)? array = yes, $($body:tt)+) => {
        impl<T, const N: usize> $($trait for)? $name<'_, T, N> $($body)+
    };
    ($name:ident<'_>, $(trait = $trait:ty,)? array = no, $($body:tt)+) => {
        impl<T> $($trait for)? $name<'_, T> $($body)+
    };
    ($name:ident<'a>, $(trait = $trait:ty,)? array = yes, $($body:tt)+) => {
        impl<'a, T, const N: usize> $($trait for)? $name<'a, T, N> $($body)+
    };
    ($name:ident<'a>, $(trait = $trait:ty,)? array = no, $($body:tt)+) => {
        impl<'a, T> $($trait for)? $name<'a, T> $($body)+
    };
    ($name:ident, $(trait = $trait:ty,)? $(arc = yes,)? array = yes, $($body:tt)+) => {
        impl<T, const N: usize> $($trait for)? $name<T, N> $($body)+
    };
    ($name:ident, $(trait = $trait:ty,)? $(arc = yes,)? array = no, $($body:tt)+) => {
        impl<T> $($trait for)? $name<T> $($body)+
    };
    ($name:ident, $(trait = $trait:ty,)? arc = no, array = yes, $($body:tt)+) => {
        impl<'a, T, const N: usize> $($trait for)? $name<'a, T, N> $($body)+
    };
    ($name:ident, $(trait = $trait:ty,)? arc = no, array = no, $($body:tt)+) => {
        impl<'a, T> $($trait for)? $name<'a, T> $($body)+
    };
}

// Ideally, this should force the user to write the actual `unsafe` keyword.
macro_rules! unsafe_impl {
    ($(#[$attr:meta])* $what:ty, for = $for:ty, array = yes $(, where $($where:tt)+)?) => {
        #[allow(clippy::undocumented_unsafe_blocks)]
        $(#[$attr])* unsafe impl<T, const N: usize> $what for $for $(where $($where)+)? {}
    };
    ($(#[$attr:meta])* $what:ty, for = $for:ty, array = no $(, where $($where:tt)+)?) => {
        #[allow(clippy::undocumented_unsafe_blocks)]
        $(#[$attr])* unsafe impl<T> $what for $for $(where $($where)+)? {}
    };
}

macro_rules! fn_ring_buffer_new {
    (arc = yes, array = yes, pow2 = $pow2:ident, module = $module:literal) => {
        /// Creates a ring buffer with a capacity of `N` and returns [`Producer`] and [`Consumer`].
        ///
        #[doc = choice!($pow2,
            "`N` must be a power of two.",
            "")]
        ///
        /// # Examples
        ///
        /// ```
        #[doc = doctest_import!($module, "RingBuffer")]
        ///
        /// let (p, c) = RingBuffer::<f32, 128>::new();
        /// ```
        ///
        /// Specifying an explicit type
        /// is is only necessary if it cannot be deduced by the compiler.
        ///
        /// ```
        #[doc = doctest_import!($module, "RingBuffer")]
        ///
        /// let (mut p, c) = RingBuffer::<_, 128>::new();
        /// assert_eq!(p.push(0.0f32), Ok(()));
        /// ```
        #[allow(clippy::new_ret_no_self)]
        pub fn new() -> (Producer<T, N>, Consumer<T, N>) {
            const {
                assert!(
                    Self::update_capacity(N) == N,
                    "`N` must be a power of two"
                );
            }
            ArcRingBuffer::new(Self::construct())
        }
    };
    (arc = yes, array = no, pow2 = $pow2:ident, module = $module:literal) => {
        /// Creates a ring buffer
        #[doc = choice!($pow2, "with at least", "with")]
        /// the given `capacity` and returns [`Producer`] and [`Consumer`].
        ///
        #[doc = choice!($pow2,
            "If the `capacity` isn't already a power of two, it is rounded up to the next one.",
            "")]
        ///
        /// # Examples
        ///
        /// ```
        #[doc = doctest_import!($module, "RingBuffer")]
        ///
        /// let (p, c) = RingBuffer::<f32>::new(100);
        /// ```
        ///
        /// Specifying an explicit type with the [turbofish](https://turbo.fish/)
        /// is is only necessary if it cannot be deduced by the compiler.
        ///
        /// ```
        #[doc = doctest_import!($module, "RingBuffer")]
        ///
        /// let (mut p, c) = RingBuffer::new(100);
        /// assert_eq!(p.push(0.0f32), Ok(()));
        /// ```
        #[allow(clippy::new_ret_no_self)]
        pub fn new(capacity: usize) -> (Producer<T>, Consumer<T>) {
            let capacity = Self::update_capacity(capacity);
            ArcRingBuffer::new(Self::construct(capacity))
        }
    };
    (arc = no, array = yes, pow2 = $pow2:ident, module = $module:literal) => {
        /// Creates a ring buffer with a capacity of `N`.
        ///
        #[doc = choice!($pow2,
            "`N` must be a power of two.",
            "")]
        ///
        /// A (single) [`Producer`] for writing into the ring buffer can be created with
        /// [`RingBuffer::producer()`].
        /// A (single) [`Consumer`] for reading from the ring buffer can be created with
        /// [`RingBuffer::consumer()`].
        ///
        /// # Examples
        ///
        /// ```
        #[doc = doctest_import!($module, "RingBuffer")]
        ///
        /// let rb = RingBuffer::<f32, 128>::new();
        /// ```
        ///
        /// Specifying an explicit type
        /// is is only necessary if it cannot be deduced by the compiler.
        ///
        /// ```
        #[doc = doctest_import!($module, "RingBuffer")]
        ///
        /// let rb = RingBuffer::<_, 128>::new();
        /// let mut p = rb.producer().unwrap();
        /// assert_eq!(p.push(0.0f32), Ok(()));
        /// ```
        pub const fn new() -> Self {
            const {
                assert!(
                    Self::update_capacity(N) == N,
                    "`N` must be a power of two"
                );
            }
            Self::construct()
        }
    };
    (arc = no, array = no, pow2 = $pow2:ident, module = $module:literal) => {
        compile_error!("TODO")
    };
}

macro_rules! fn_ring_buffer_drop_all_elements {
    (bip = yes) => {
        fn_ring_buffer_drop_all_elements_helper!(skip);
    };
    (bip = no) => {
        fn_ring_buffer_drop_all_elements_helper!();
    };
}
macro_rules! fn_ring_buffer_drop_all_elements_helper {
    ($($skip:ident)?) => {
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
            $(
                let $skip = self.skip.load(Ordering::Relaxed);
            )?
            // Loop over all slots that hold a value and drop them.
            while head != tail {
                $(
                    if self.collapse_position(head) == $skip {
                        head = self.increment(head, self.capacity() - $skip);
                    }
                )?
                // SAFETY: All slots between head and tail have been initialized.
                unsafe { self.slot_ptr(head).drop_in_place() };
                head = self.increment1(head);
            }
        }
    };
}

macro_rules! fn_ring_buffer_update_capacity {
    (pow2 = yes) => {
        const fn update_capacity(capacity: usize) -> usize {
            capacity.next_power_of_two()
        }
    };
    (pow2 = no) => {
        const fn update_capacity(capacity: usize) -> usize {
            capacity
        }
    };
}

/// Makes sure the position is in the range `0 .. capacity`.
macro_rules! fn_ring_buffer_collapse_position {
    (pow2 = yes) => {
        // Wraps from any number to the range `0 .. capacity`.
        fn collapse_position(&self, pos: usize) -> usize {
            // TODO: is capacity 0 supported?
            pos & (self.capacity() - 1)
        }
    };
    (pow2 = no) => {
        // Wraps from the range `0 .. 2 * capacity` to `0 .. capacity`.
        fn collapse_position(&self, pos: usize) -> usize {
            debug_assert!(pos == 0 || pos < 2 * self.capacity());
            if pos < self.capacity() {
                pos
            } else {
                pos - self.capacity()
            }
        }
    };
}

macro_rules! fn_ring_buffer_slot_ptr {
    () => {
        unsafe fn slot_ptr(&self, pos: usize) -> *mut T {
            // SAFETY: See docstring.
            unsafe { self.data_ptr().add(self.collapse_position(pos)) }
        }
    };
}

/// Increments a position by going `n` slots forward.
macro_rules! fn_ring_buffer_increment {
    (pow2 = yes) => {
        fn increment(&self, pos: usize, n: usize) -> usize {
            pos.wrapping_add(n)
        }
    };
    (pow2 = no) => {
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
    };
}

/// Increments a position by going one slot forward.
///
/// This might be more efficient than self.increment(..., 1).
macro_rules! fn_ring_buffer_increment1 {
    (pow2 = yes) => {
        fn increment1(&self, pos: usize) -> usize {
            pos.wrapping_add(1)
        }
    };
    (pow2 = no) => {
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
    };
}

/// Returns the distance between two positions.
macro_rules! fn_ring_buffer_distance {
    (pow2 = yes) => {
        fn distance(&self, a: usize, b: usize) -> usize {
            b.wrapping_sub(a)
        }
    };
    (pow2 = no) => {
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

macro_rules! struct_arc_ring_buffer {
    (arc = yes, array = $array:ident) => {
        use alloc::boxed::Box;
        use core::ptr::NonNull;

        // Non-public helper type.
        //#[derive(Debug, PartialEq, Eq)]
        // TODO: make non-public!
        struct_!(ArcRingBuffer, array = $array, params = (T), fields = {
            ptr: NonNull<ty!(RingBuffer, array = $array)>,
        });

        // SAFETY: If RingBuffer is Send, ArcRingBuffer is as well.
        unsafe_impl!(
            Send,
            for = ty!(ArcRingBuffer, array = $array),
            array = $array,
            where ty!(RingBuffer, array = $array): Send);

        impl_!(ArcRingBuffer, array = $array, {
            #[allow(clippy::new_ret_no_self)]
            fn new(
                rb: ty!(RingBuffer, array = $array)
            ) -> (ty!(Producer, array = $array), ty!(Consumer, array = $array)) {
                debug_assert_eq!(rb.flags.load(Ordering::Relaxed) & IS_ABANDONED, 0);
                let head = rb.head.load(Ordering::Relaxed);
                let tail = rb.tail.load(Ordering::Relaxed);
                let ptr = Box::leak(Box::new(rb));
                // SAFETY: Pointer from Box is always non-null.
                let ptr = unsafe { NonNull::new_unchecked(ptr) };
                let p = Producer {
                    buffer: Self { ptr },
                    cached_head: Cell::new(head),
                    cached_tail: Cell::new(tail),
                };
                let c = Consumer {
                    buffer: Self { ptr },
                    cached_head: Cell::new(head),
                    cached_tail: Cell::new(tail),
                };
                (p, c)
            }
        });

        impl_!(ArcRingBuffer, trait = Drop, array = $array, {
            fn drop(&mut self) {
                // SAFETY: must point to initialized Storage.
                let flags: &AtomicU8 = unsafe { &self.ptr.as_ref().flags };
                // The "store" part of `fetch_or()` has to use `Release` to make sure that any previous writes
                // to the ring buffer happen before it (in the thread that drops first).
                // The "load" part can be `Relaxed` for the first thread,
                // but it must be `Acquire` for the second one (see below).
                if flags.fetch_or(IS_ABANDONED, Ordering::Release) & IS_ABANDONED == 0 {
                    // The flag wasn't set before, so we are the first to drop our
                    // producer/consumer and it should not be dropped yet.
                } else {
                    // The flag was already set, i.e. the other thread has already dropped its
                    // consumer/producer and it can be dropped now.

                    // However, since the load of `flags` was `Relaxed`,
                    // we have to use `Acquire` here to make sure that reading `head` and `tail`
                    // in the destructor happens after this point.

                    // Ideally, we would use a memory fence like this:
                    //core::sync::atomic::fence(Ordering::Acquire);
                    // ... but as long as ThreadSanitizer doesn't support fences,
                    // we use load(Acquire) as a work-around to avoid false positives:
                    let _ = flags.load(Ordering::Acquire);
                    // SAFETY: RingBuffer has been allocated with `Box::new()`.
                    unsafe {
                        drop_slow(self.ptr);
                    }
                }
            }
        });

        fn_arc_ring_buffer_drop_slow!(array = $array);

        impl_!(ArcRingBuffer, trait = core::ops::Deref, array = $array, {
            type Target = ty!(RingBuffer, array = $array);

            fn deref(&self) -> &Self::Target {
                // SAFETY: There are never any mutable references.
                unsafe { self.ptr.as_ref() }
            }
        });
    };
    (arc = no, array = $array:ident) => {};
}

macro_rules! fn_arc_ring_buffer_drop_slow_helper {
    (params = ($($params:tt)*), args = ($($args:tt)*)) => {
        /// Non-inlined part of `Ref::drop()`.
        #[inline(never)]
        unsafe fn drop_slow<$($params)*>(ptr: NonNull<RingBuffer<$($args)*>>) {
            // SAFETY: This is allowed because the storage has been allocated with `Box::new()`.
            unsafe {
                // Turn the pointer back into a `Box` and immediately drop it,
                // which deallocates the memory allocated in `Ref::new()`.
                drop(Box::from_raw(ptr.as_ptr()));
            }
        }
    };
}

macro_rules! fn_arc_ring_buffer_drop_slow {
    (array = yes) => {
        fn_arc_ring_buffer_drop_slow_helper!(params = (T, const N: usize), args = (T, N));
    };
    (array = no) => {
        fn_arc_ring_buffer_drop_slow_helper!(params = (T), args = (T));
    };
}

macro_rules! ty {
    ($name:ident, array = yes$(, $a:lifetime)?) => {
        $name<$($a, )?T, N>
    };
    ($name:ident, array = no$(, $a:lifetime)?) => {
        $name<$($a, )?T>
    };
    ($name:ident, arc = yes, array = yes) => {
        $name<T, N>
    };
    ($name:ident, arc = yes, array = no) => {
        $name<T>
    };
    ($name:ident, arc = no, array = yes) => {
        $name<'a, T, N>
    };
    ($name:ident, arc = no, array = no) => {
        $name<'a, T>
    };
}

macro_rules! fn_ring_buffer_producer {
    (arc = yes, array = $array:ident) => {};
    (arc = no, array = $array:ident) => {
        pub fn producer(&self) -> Option<ty!(Producer, array = $array)> {
            let old_flags = self.flags.fetch_or(HAS_PRODUCER, Ordering::SeqCst);
            if old_flags & HAS_PRODUCER == 0 {
                let head = self.head.load(Ordering::Relaxed);
                let tail = self.tail.load(Ordering::Relaxed);
                Some(Producer {
                    buffer: self,
                    cached_head: Cell::new(head),
                    cached_tail: Cell::new(tail),
                })
            } else {
                None
            }
        }
    };
}

macro_rules! fn_ring_buffer_consumer {
    (arc = yes, array = $array:ident) => {};
    (arc = no, array = $array:ident) => {
        pub fn consumer(&self) -> Option<ty!(Consumer, array = $array)> {
            let old_flags = self.flags.fetch_or(HAS_CONSUMER, Ordering::SeqCst);
            if old_flags & HAS_CONSUMER == 0 {
                let head = self.head.load(Ordering::Relaxed);
                let tail = self.tail.load(Ordering::Relaxed);
                Some(Consumer {
                    buffer: self,
                    cached_head: Cell::new(head),
                    cached_tail: Cell::new(tail),
                })
            } else {
                None
            }
        }
    };
}

macro_rules! struct_producer_docstring {
    () => { docstring!(
        /// The producer side of a [`RingBuffer`].
        ///
        /// A `Producer` can be moved between threads,
        /// but references from different threads are not allowed
        /// (i.e. it is [`Send`] but not [`Sync`]).
        ///
        /// Individual elements can be moved into the ring buffer with [`Producer::push()`],
        /// multiple elements at once can be written with [`Producer::write_chunk()`]
        /// and [`Producer::write_chunk_uninit()`].
        ///
        /// The number of free slots currently available for writing can be obtained with
        /// [`Producer::slots()`].
    )};
}

macro_rules! struct_producer {
    (arc = yes, array = $array:ident) => {
        // TODO: manual impls:
        //#[derive(Debug, PartialEq, Eq)]
        struct_!(
            #[doc = struct_producer_docstring!()]
            ///
            /// A `Producer` can only be created with [`RingBuffer::new()`]
            /// (together with its counterpart, the [`Consumer`]).
            ///
            /// When the `Producer` is dropped, [`Consumer::is_abandoned()`] will return `true`.
            /// This can be used as a crude way to communicate to the receiving thread
            /// that no more data will be produced.
            /// When the `Producer` is dropped after the [`Consumer`] has already been dropped,
            /// all items remaining in the ring buffer will be dropped and the allocated memory
            /// will be deallocated.
            Producer, array = $array, params = (T), fields = {
                buffer: ty!(ArcRingBuffer, array = $array),
                cached_head: Cell<usize>,
                cached_tail: Cell<usize>,
            }
        );
    };
    (arc = no, array = $array:ident) => {
        // TODO: manual impls:
        //#[derive(Debug, PartialEq, Eq)]
        struct_!(
            #[doc = struct_producer_docstring!()]
            ///
            /// A `Producer` can only be created with [`RingBuffer::producer()`].
            Producer, array = $array, params = ('a, T), fields = {
                buffer: &'a ty!(RingBuffer, array = $array),
                cached_head: Cell<usize>,
                cached_tail: Cell<usize>,
            }
        );

        impl_!(Producer<'_>, trait = Drop, array = $array, {
            fn drop(&mut self) {
                let _ = self.buffer.flags.fetch_and(!HAS_PRODUCER, Ordering::SeqCst);
            }
        });
    };
}

macro_rules! struct_consumer_docstring {
    () => { docstring!(
        /// The consumer side of a [`RingBuffer`].
        ///
        /// A `Consumer` can be moved between threads,
        /// but references from different threads are not allowed
        /// (i.e. it is [`Send`] but not [`Sync`]).
        ///
        /// Individual elements can be moved out of the ring buffer with [`Consumer::pop()`],
        /// multiple elements at once can be read with [`Consumer::read_chunk()`].
        ///
        /// The number of slots currently available for reading can be obtained with
        /// [`Consumer::slots()`].
    )};
}

macro_rules! struct_consumer {
    (arc = yes, array = $array:ident) => {
        // TODO: manual impls:
        //#[derive(Debug, PartialEq, Eq)]
        struct_!(
            #[doc = struct_consumer_docstring!()]
            ///
            /// A `Consumer` can only be created with [`RingBuffer::new()`]
            /// (together with its counterpart, the [`Producer`]).
            ///
            /// When the `Consumer` is dropped, [`Producer::is_abandoned()`] will return `true`.
            /// This can be used as a crude way to communicate to the sending thread
            /// that no more data will be consumed.
            /// When the `Consumer` is dropped after the [`Producer`] has already been dropped,
            /// all items remaining in the ring buffer will be dropped and the allocated memory
            /// will be deallocated.
            Consumer, array = $array, params = (T), fields = {
                buffer: ty!(ArcRingBuffer, array = $array),
                cached_head: Cell<usize>,
                cached_tail: Cell<usize>,
            }
        );
    };
    (arc = no, array = $array:ident) => {
        // TODO: manual impls:
        //#[derive(Debug, PartialEq, Eq)]
        struct_!(
            #[doc = struct_consumer_docstring!()]
            ///
            /// A `Consumer` can only be created with [`RingBuffer::consumer()`].
            Consumer, array = $array, params = ('a, T), fields = {
                buffer: &'a ty!(RingBuffer, array = $array),
                cached_head: Cell<usize>,
                cached_tail: Cell<usize>,
            }
        );

        impl_!(Consumer<'_>, trait = Drop, array = $array, {
            fn drop(&mut self) {
                let _ = self.buffer.flags.fetch_and(!HAS_CONSUMER, Ordering::SeqCst);
            }
        });
    };
}

macro_rules! fn_producer_push {
    (arc = $arc:ident, array = $array:ident, module = $module:literal) => {
        /// Attempts to push an element into the queue.
        ///
        /// The element is *moved* into the ring buffer and its slot
        /// is made available to be read by the [`Consumer`].
        ///
        /// # Errors
        ///
        /// If the queue is full, the element is returned back as an error.
        ///
        /// # Examples
        ///
        /// ```
        #[doc = doctest_import!($module, "{PushError, RingBuffer}")]
        ///
        #[doc = doctest_create_ring_buffer!(arc = $arc, array = $array, capacity = 1)]
        ///
        /// assert_eq!(p.push(10), Ok(()));
        /// assert_eq!(p.push(20), Err(PushError::Full(20)));
        /// ```
        pub fn push(&mut self, value: T) -> Result<(), PushError<T>> {
            if let Some(tail) = self.next_tail() {
                let b = &self.buffer;
                // SAFETY: tail points to an empty slot.
                unsafe { b.slot_ptr(tail).write(value) };
                let tail = b.increment1(tail);
                b.tail.store(tail, Ordering::Release);
                self.cached_tail.set(tail);
                Ok(())
            } else {
                Err(PushError::Full(value))
            }
        }
    };
}

macro_rules! fn_producer_slots {
    (arc = $arc:ident, array = $array:ident, module = $module:literal) => {
        /// Returns the number of slots available for writing.
        ///
        /// Since items can be concurrently consumed on another thread, the actual number
        /// of available slots may increase at any time
        /// (up to the [`capacity()`](Producer::capacity)).
        ///
        /// To check for a single available slot,
        /// using [`is_full()`](Producer::is_full) is often quicker
        /// (because it might not have to check an atomic variable).
        ///
        /// # Examples
        ///
        /// ```
        #[doc = doctest_import!($module, "RingBuffer")]
        ///
        #[doc = doctest_create_ring_buffer!(arc = $arc, array = $array, capacity = 1024)]
        /// assert_eq!(p.push(0.5f32), Ok(()));
        ///
        /// assert_eq!(p.slots(), 1023);
        /// ```
        pub fn slots(&self) -> usize {
            let b = &self.buffer;
            let head = b.head.load(Ordering::Acquire);
            self.cached_head.set(head);
            b.capacity() - b.distance(head, self.cached_tail.get())
        }
    };
}

macro_rules! fn_producer_slots_contiguousX {
    (bip = yes) => {
        // TODO: inline?
        fn slots_contiguous_something(&self) -> (usize, bool) {
            // TODO: code reuse with write_chunk_uninit() and next_tail()
            let b = &self.buffer;
            let mut head = self.cached_head.get();
            let tail = self.cached_tail.get();
            let is_empty = head == tail;
            let mut collapsed_head = b.collapse_position(head);
            let collapsed_tail = b.collapse_position(tail);
            if is_empty || collapsed_head < collapsed_tail {
                let slots = b.capacity() - collapsed_tail;
                debug_assert!(slots != 0 || b.capacity() == 0);
                return (slots, false);
            }
            head = b.head.load(Ordering::Acquire);
            self.cached_head.set(head);
            collapsed_head = b.collapse_position(head);
            (collapsed_head - collapsed_tail, true)
        }

        pub fn slots_without_skipping(&self) -> usize {
            self.slots_contiguous_something().0
        }

        pub fn slots_contiguous_with_potential_followup(&self) -> (usize, usize) {
            let (slots, refreshed) = self.slots_contiguous_something();
            if refreshed {
                return (slots, 0);
            }
            let b = &self.buffer;
            let head = b.head.load(Ordering::Acquire);
            self.cached_head.set(head);
            let tail = self.cached_tail.get();
            (slots, b.collapse_position(head) - b.collapse_position(tail))
        }

        /// The maximum number of slots that write_chunk() ... can provide.
        ///
        /// ... this can change at any time, up to ..., depending on ...
        ///
        /// ... using this value might lead to skipping ... use ... to avoid ...
        pub fn slots_contiguous_max(&self) -> usize {
            let (one, two) = self.slots_contiguous_with_potential_followup();
            one.max(two)
        }
    };
    (bip = no) => {};
}

macro_rules! fn_producer_is_full {
    (arc = $arc:ident, array = $array:ident, module = $module:literal) => {
        /// Returns `true` if there are currently no slots available for writing.
        ///
        /// TODO: additional info about bip?
        ///
        /// A full ring buffer might cease to be full at any time
        /// if the corresponding [`Consumer`] is consuming items in another thread.
        ///
        /// # Examples
        ///
        /// ```
        #[doc = doctest_import!($module, "RingBuffer")]
        ///
        #[doc = doctest_create_ring_buffer!(arc = $arc, array = $array, capacity = 1)]
        ///
        /// assert!(!p.is_full());
        /// assert_eq!(p.push(10), Ok(()));
        /// assert!(p.is_full());
        /// ```
        ///
        /// Since items can be concurrently consumed on another thread, the ring buffer
        /// might not be full for long:
        ///
        /// ```
        #[doc = doctest_import!($module, "RingBuffer", "# ")]
        #[doc = doctest_create_ring_buffer!(arc = $arc, array = $array, capacity = 1, "# ")]
        /// # assert_eq!(p.push(10), Ok(()));
        /// if p.is_full() {
        ///     // The buffer might be full, but it might as well not be
        ///     // if an item was just consumed on another thread.
        /// }
        /// ```
        ///
        /// However, if it's not full, another thread cannot change that:
        ///
        /// ```
        #[doc = doctest_import!($module, "RingBuffer", "# ")]
        #[doc = doctest_create_ring_buffer!(arc = $arc, array = $array, capacity = 1, "# ")]
        /// # assert_eq!(p.push(10), Ok(()));
        /// if !p.is_full() {
        ///     // At least one slot is guaranteed to be available for writing.
        /// }
        /// ```
        ///
        /// TODO: example for "bip" when "skip" is set?
        pub fn is_full(&self) -> bool {
            self.next_tail().is_none()
        }
    };
}

macro_rules! fn_pc_capacity {
    (arc = $arc:ident, array = $array:ident, module = $module:literal) => {
        /// Returns the total capacity of the queue.
        ///
        /// At any time, the capacity is subdivided into
        /// [`Producer::slots()`] available for writing and
        /// [`Consumer::slots()`] available for reading.
        ///
        /// TODO: not quite true for bip
        ///
        /// # Examples
        ///
        /// ```
        #[doc = doctest_import!($module, "RingBuffer")]
        ///
        #[doc = doctest_create_ring_buffer!(arc = $arc, array = $array, capacity = 128)]
        ///
        /// assert_eq!(p.push(-0.7), Ok(()));
        /// assert_eq!(p.slots(), 127);
        /// assert_eq!(c.slots(), 1);
        /// assert_eq!(p.capacity(), 128);
        /// assert_eq!(c.capacity(), 128);
        /// ```
        pub fn capacity(&self) -> usize {
            self.buffer.capacity()
        }
    };
}

/// NB: next_tail() can also be used for "bip", because `b.skip` is never set.
/// One element can always be inserted without skipping.
macro_rules! fn_producer_next_tail {
    () => {
        /// Get the tail position for writing the next slot, if available.
        ///
        /// This is a strict subset of the functionality implemented in `write_chunk_uninit()`.
        /// For performance, this special case is implemented separately.
        fn next_tail(&self) -> Option<usize> {
            let head = self.cached_head.get();
            let tail = self.cached_tail.get();
            let b = &self.buffer;
            // Check if the queue is *possibly* full.
            if b.distance(head, tail) == b.capacity() {
                // Refresh the head ...
                let head = b.head.load(Ordering::Acquire);
                self.cached_head.set(head);
                // ... and check if it's *really* full.
                if b.distance(head, tail) == b.capacity() {
                    // `head` didn't change, queue is full.
                    return None;
                }
            }
            Some(tail)
        }
    };
}

macro_rules! fn_consumer_pop {
    (arc = $arc:ident, array = $array:ident, module = $module:literal) => {
        /// Attempts to pop an element from the queue.
        ///
        /// The element is *moved* out of the ring buffer and its slot
        /// is made available to be filled by the [`Producer`] again.
        ///
        /// # Errors
        ///
        /// If the queue is empty, an error is returned.
        ///
        /// # Examples
        ///
        /// ```
        #[doc = doctest_import!($module, "{PopError, RingBuffer}")]
        ///
        #[doc = doctest_create_ring_buffer!(arc = $arc, array = $array, capacity = 1)]
        ///
        /// assert_eq!(p.push(10), Ok(()));
        /// assert_eq!(c.pop(), Ok(10));
        /// assert_eq!(c.pop(), Err(PopError::Empty));
        /// ```
        ///
        /// To obtain an [`Option<T>`](Option), use [`.ok()`](Result::ok) on the result.
        ///
        /// ```
        #[doc = doctest_import!($module, "RingBuffer", "# ")]
        #[doc = doctest_create_ring_buffer!(arc = $arc, array = $array, capacity = 1, "# ")]
        /// assert_eq!(p.push(20), Ok(()));
        /// assert_eq!(c.pop().ok(), Some(20));
        /// ```
        pub fn pop(&mut self) -> Result<T, PopError> {
            if let Some(head) = self.next_head() {
                let b = &self.buffer;
                // SAFETY: head points to an initialized slot.
                let value = unsafe { b.slot_ptr(head).read() };
                let head = b.increment1(head);
                b.head.store(head, Ordering::Release);
                self.cached_head.set(head);
                Ok(value)
            } else {
                Err(PopError::Empty)
            }
        }
    };
}

macro_rules! fn_consumer_peek {
    (arc = $arc:ident, array = $array:ident, module = $module:literal) => {
        /// Attempts to read an element from the queue without removing it.
        ///
        /// # Errors
        ///
        /// If the queue is empty, an error is returned.
        ///
        /// # Examples
        ///
        /// ```
        #[doc = doctest_import!($module, "{PeekError, RingBuffer}")]
        ///
        #[doc = doctest_create_ring_buffer!(arc = $arc, array = $array, capacity = 1)]
        ///
        /// assert_eq!(c.peek(), Err(PeekError::Empty));
        /// assert_eq!(p.push(10), Ok(()));
        /// assert_eq!(c.peek(), Ok(&10));
        /// assert_eq!(c.peek(), Ok(&10));
        /// ```
        pub fn peek(&self) -> Result<&T, PeekError> {
            if let Some(head) = self.next_head() {
                // SAFETY: head points to an initialized slot.
                Ok(unsafe { &*self.buffer.slot_ptr(head) })
            } else {
                Err(PeekError::Empty)
            }
        }
    };
}

macro_rules! fn_consumer_slots_docstring {
    (arc = $arc:ident, array = $array:ident, module = $module:literal) => { docstring!(
        /// Returns the number of slots available for reading.
        ///
        /// Since items can be concurrently produced on another thread, the actual number
        /// of available slots may increase at any time
        /// (up to the [`capacity()`](Consumer::capacity)).
        ///
        /// To check for a single available slot,
        /// using [`is_empty()`](Consumer::is_empty) is often quicker
        /// (because it might not have to check an atomic variable).
        ///
        /// TODO: insert bip specifics
        ///
        /// # Examples
        ///
        /// ```
        #[doc = doctest_import!($module, "RingBuffer")]
        #[doc = doctest_create_ring_buffer!(arc = $arc, array = $array, capacity = 1024)]
        ///
        /// assert_eq!(c.slots(), 0);
        /// assert_eq!(p.push(0.0), Ok(()));
        /// assert_eq!(c.slots(), 1);
        /// ```
    )};
}

macro_rules! fn_consumer_slots {
    (arc = $arc:ident, array = $array:ident, bip = yes, module = $module:literal) => {
        #[doc = fn_consumer_slots_docstring!(arc = $arc, array = $array, module = $module)]
        ///
        /// TODO: [`read_chunk()`](Consumer::read_chunk) might not provide the full number of free slots
        ///
        /// TODO: see alternative "slots" variations
        pub fn slots(&self) -> usize {
            let b = &self.buffer;
            let head = self.cached_head.get();
            let tail = b.tail.load(Ordering::Acquire);
            self.cached_tail.set(tail);
            if head == tail {
                return 0;
            }
            let collapsed_head = b.collapse_position(head);
            let collapsed_tail = b.collapse_position(tail);
            if collapsed_head < collapsed_tail {
                collapsed_tail - collapsed_head
            } else {
                let skip = b.skip.load(Ordering::Acquire);
                collapsed_tail + skip - collapsed_head
            }
        }
    };
    (arc = $arc:ident, array = $array:ident, bip = no, module = $module:literal) => {
        #[doc = fn_consumer_slots_docstring!(arc = $arc, array = $array, module = $module)]
        pub fn slots(&self) -> usize {
            let b = &self.buffer;
            let tail = b.tail.load(Ordering::Acquire);
            self.cached_tail.set(tail);
            b.distance(self.cached_head.get(), tail)
        }
    };
}

macro_rules! fn_consumer_slots_contiguousX {
    (bip = yes) => {
        fn slots_contiguous_helper(&self) -> (usize, bool) {
            // TODO: code reuse with read_chunk() and next_head()?
            let b = &self.buffer;
            let mut head = self.cached_head.get();
            let mut tail = self.cached_tail.get();
            let mut slots = 0;
            let mut is_empty = head == tail;
            let mut collapsed_head = b.collapse_position(head);
            let mut collapsed_tail = b.collapse_position(tail);
            if !is_empty && collapsed_tail <= collapsed_head {
                // NB: We are only allowed to use `skip` if (collapsed) `tail < head`
                //     (or if the buffer is full).
                let skip = b.skip.load(Ordering::Acquire);
                slots = skip - collapsed_head;
                if slots != 0 {
                    return (slots, false);
                }
                if skip != b.capacity() {
                    b.skip.store(b.capacity(), Ordering::Release);
                }
                head = b.increment(head, b.capacity() - collapsed_head);
                // NB: `skip` is stored before `head`.
                b.head.store(head, Ordering::Release);
                self.cached_head.set(head);
                collapsed_head = b.collapse_position(head);
            } else {
                // nothing to do here, we have to refresh tail
            }
            tail = b.tail.load(Ordering::Acquire);
            self.cached_tail.set(tail);
            collapsed_tail = b.collapse_position(tail);
            is_empty = head == tail;
            if is_empty || collapsed_head < collapsed_tail {
                debug_assert_eq!(slots, 0);
                (collapsed_tail - collapsed_head, true)
            } else {
                (collapsed_tail, true)
            }
        }

        /// Returns the maximum number of slots that are available for reading with
        /// [`read_chunk()`](Consumer::read_chunk).
        ///
        /// ... this can change when the producer is writing stuff ...
        ///
        /// ... might be smaller than [`slots()`](Consumer::slots) because ...
        // TODO: better name?
        pub fn slots_contiguous_first(&self) -> usize {
            self.slots_contiguous_helper().0
        }

        /// Returns the maximum number of slots that are available for reading with
        /// [`read_chunk()`](Consumer::read_chunk) *twice*.
        ///
        /// The first number is the same as ... (which might be slightly more efficient)
        ///
        /// If the first number is `0`, the second is `0` as well.
        // TODO: better name?
        pub fn slots_contiguous_with_potential_followup(&self) -> (usize, usize) {
            let (slots, refreshed) = self.slots_contiguous_helper();
            if refreshed {
                return (slots, 0);
            }
            let b = &self.buffer;
            let head = self.cached_head.get();
            let collapsed_head = b.collapse_position(head);
            let tail = b.tail.load(Ordering::Acquire);
            self.cached_tail.set(tail);
            let collapsed_tail = b.collapse_position(tail);
            let is_empty = head == tail;
            if is_empty || collapsed_head < collapsed_tail {
                debug_assert_eq!(slots, 0);
                (collapsed_tail - collapsed_head, 0)
            } else {
                (slots, collapsed_tail)
            }
        }
    };
    (bip = no) => {};
}

macro_rules! fn_consumer_is_empty {
    (arc = $arc:ident, array = $array:ident, module = $module:literal) => {
        /// Returns `true` if there are currently no slots available for reading.
        ///
        /// TODO: additional info about bip?
        ///
        /// An empty ring buffer might cease to be empty at any time
        /// if the corresponding [`Producer`] is producing items in another thread.
        ///
        /// # Examples
        ///
        /// ```
        #[doc = doctest_import!($module, "RingBuffer")]
        ///
        #[doc = doctest_create_ring_buffer!(arc = $arc, array = $array, capacity = 1)]
        ///
        /// assert!(c.is_empty());
        /// assert_eq!(p.push(0.0), Ok(()));
        /// assert!(!c.is_empty());
        /// ```
        ///
        /// Since items can be concurrently produced on another thread, the ring buffer
        /// might not be empty for long:
        ///
        /// ```
        #[doc = doctest_import!($module, "RingBuffer", "# ")]
        #[doc = doctest_create_ring_buffer!(arc = $arc, array = $array, capacity = 1, "# ")]
        /// # assert_eq!(p.push(0.0), Ok(()));
        /// if c.is_empty() {
        ///     // The buffer might be empty, but it might as well not be
        ///     // if an item was just produced on another thread.
        /// }
        /// ```
        ///
        /// However, if it's not empty, another thread cannot change that:
        ///
        /// ```
        #[doc = doctest_import!($module, "RingBuffer", "# ")]
        #[doc = doctest_create_ring_buffer!(arc = $arc, array = $array, capacity = 1, "# ")]
        /// # assert_eq!(p.push(0.0), Ok(()));
        /// if !c.is_empty() {
        ///     // At least one slot is guaranteed to be available for reading.
        /// }
        /// ```
        pub fn is_empty(&self) -> bool {
            self.next_head().is_none()
        }
    };
}

macro_rules! fn_producer_is_abandoned {
    (arc = yes, array = $array:ident, module = $module:literal) => {
        /// Returns `true` if the corresponding [`Consumer`] has been destroyed.
        ///
        /// TODO: update this note:
        ///
        /// Note that since Rust version 1.74.0, this is not synchronizing with the consumer thread
        /// anymore, see <https://github.com/mgeier/rtrb/issues/114>.
        /// In a future version of `rtrb`, the synchronizing behavior might be restored.
        ///
        /// # Examples
        ///
        /// ```
        #[doc = doctest_import!($module, "RingBuffer")]
        ///
        #[doc = doctest_create_ring_buffer!(arc = yes, array = $array, capacity = 7)]
        /// assert!(!p.is_abandoned());
        /// assert_eq!(p.push(10), Ok(()));
        /// drop(c);
        /// // The items that are still in the ring buffer are not accessible anymore.
        /// assert!(p.is_abandoned());
        /// // Even though it's futile, items can still be written:
        /// assert_eq!(p.push(11), Ok(()));
        /// ```
        ///
        /// Since the consumer can be concurrently dropped on another thread,
        /// the producer might become abandoned at any time:
        ///
        /// ```
        #[doc = doctest_import!($module, "RingBuffer", "# ")]
        #[doc = doctest_create_ring_buffer!(arc = yes, array = $array, capacity = 1, "# ")]
        /// # assert_eq!(p.push(10), Ok(()));
        /// if !p.is_abandoned() {
        ///     // Right now, the consumer might still be alive, but it might as well not be
        ///     // if another thread has just dropped it.
        /// }
        /// ```
        ///
        /// However, if it already is abandoned, it will stay that way:
        ///
        /// ```
        #[doc = doctest_import!($module, "RingBuffer", "# ")]
        #[doc = doctest_create_ring_buffer!(arc = yes, array = $array, capacity = 1, "# ")]
        /// # assert_eq!(p.push(10), Ok(()));
        /// if p.is_abandoned() {
        ///     // The consumer does definitely not exist anymore.
        /// }
        /// ```
        pub fn is_abandoned(&self) -> bool {
            self.buffer.flags.load(Ordering::Acquire) & IS_ABANDONED != 0
        }
    };
    (arc = no, array = $array:ident, module = $module:literal) => {};
}

macro_rules! fn_consumer_is_abandoned {
    (arc = yes, array = $array:ident, module = $module:literal) => {
        /// Returns `true` if the corresponding [`Producer`] has been destroyed.
        ///
        /// TODO: update this note:
        ///
        /// Note that since Rust version 1.74.0, this is not synchronizing with the producer thread
        /// anymore, see <https://github.com/mgeier/rtrb/issues/114>.
        /// In a future version of `rtrb`, the synchronizing behavior might be restored.
        ///
        /// # Examples
        ///
        /// ```
        #[doc = doctest_import!($module, "RingBuffer")]
        ///
        #[doc = doctest_create_ring_buffer!(arc = yes, array = $array, capacity = 7)]
        /// assert!(!c.is_abandoned());
        /// assert_eq!(p.push(10), Ok(()));
        /// drop(p);
        /// assert!(c.is_abandoned());
        /// // The items that are left in the ring buffer can still be consumed:
        /// assert_eq!(c.pop(), Ok(10));
        /// ```
        ///
        /// Since the producer can be concurrently dropped on another thread,
        /// the consumer might become abandoned at any time:
        ///
        /// ```
        #[doc = doctest_import!($module, "RingBuffer", "# ")]
        #[doc = doctest_create_ring_buffer!(arc = yes, array = $array, capacity = 1, "# ")]
        /// # assert_eq!(p.push(10), Ok(()));
        /// if !c.is_abandoned() {
        ///     // Right now, the producer might still be alive, but it might as well not be
        ///     // if another thread has just dropped it.
        /// }
        /// ```
        ///
        /// However, if it already is abandoned, it will stay that way:
        ///
        /// ```
        #[doc = doctest_import!($module, "RingBuffer", "# ")]
        #[doc = doctest_create_ring_buffer!(arc = yes, array = $array, capacity = 1, "# ")]
        /// # assert_eq!(p.push(10), Ok(()));
        /// if c.is_abandoned() {
        ///     // The producer does definitely not exist anymore.
        /// }
        /// ```
        pub fn is_abandoned(&self) -> bool {
            self.buffer.flags.load(Ordering::Acquire) & IS_ABANDONED != 0
        }
    };
    (arc = no, array = $array:ident, module = $module:literal) => {};
}

/// Get the `head` position for reading the next slot, if available.
///
/// This is a strict subset of the functionality implemented in `read_chunk()`.
/// For performance, this special case is implemented separately.
macro_rules! fn_consumer_next_head {
    (bip = yes) => {
        fn next_head(&self) -> Option<usize> {
            // NB: cached_head is always up-to-date, no need for atomic load here.
            let mut head = self.cached_head.get();
            let mut tail = self.cached_tail.get();
            let b = &self.buffer;

            // Check if the queue is *possibly* empty.
            if head == tail {
                // Refresh the tail ...
                tail = b.tail.load(Ordering::Acquire);
                self.cached_tail.set(tail);
                // ... and check if it's *really* empty.
                if head == tail {
                    // `tail` didn't change, queue is empty.
                    return None;
                } else if b.collapse_position(head) < b.collapse_position(tail) {
                    // `tail` did change, but it didn't wrap around.
                    return Some(head);
                }
            } else if head < tail {
                // The tail might have wrapped around in the meantime.
                tail = b.tail.load(Ordering::Acquire);
                self.cached_tail.set(tail);
            } else {
                // The tail cannot overtake the head, no need to refresh at this point.
            }
            debug_assert_ne!(head, tail);
            if b.collapse_position(tail) < b.collapse_position(head) {
                // NB: We are only allowed to use `skip` if (collapsed) `tail < head`.
                let skip = b.skip.load(Ordering::Acquire);
                if b.collapse_position(head) == skip {
                    // Nothing to read at the end of the buffer, wrap `head` and clear `skip`.
                    head = b.increment(head, b.capacity() - skip);
                    b.head.store(head, Ordering::Release);
                    self.cached_head.set(head);
                    b.skip.store(b.capacity(), Ordering::Release);

                    // NB: The producer only sets `skip` if it writes at least one slot
                    // at the beginning of the buffer.  Therefore, we know that the
                    // wrapped-around `head` is valid for reading (at least) one slot.
                }
            }
            Some(head)
        }
    };
    (bip = no) => {
        fn next_head(&self) -> Option<usize> {
            let head = self.cached_head.get();
            let tail = self.cached_tail.get();

            // Check if the queue is *possibly* empty.
            if head == tail {
                // Refresh the tail ...
                let tail = self.buffer.tail.load(Ordering::Acquire);
                self.cached_tail.set(tail);
                // ... and check if it's *really* empty.
                if head == tail {
                    // `tail` didn't change, queue is empty.
                    return None;
                }
            }
            Some(head)
        }
    };
}

macro_rules! fn_write_chunk_uninit_commit_unchecked {
    (bip = yes) => {
        unsafe fn commit_unchecked(self, n: usize) -> usize {
            if n == 0 {
                // NB: No slots will be skipped, both `tail` and `skip` remain unchanged.
                // This is the same as if the function wasn't called at all.
                return n;
            }
            let b = &self.producer.buffer;
            let mut tail = self.producer.cached_tail.get();
            let collapsed_tail = b.collapse_position(tail);
            if self.ptr == b.data_ptr() && collapsed_tail != 0 {
                // NB: It is safe to store `skip` before `tail`, because the consumer
                // will potentially only read between `head` and (the old) `tail`,
                // without looking at `skip`.
                // Storing `tail` before `skip` would be problematic, however, because
                // the consumer would see new data at the beginning of the buffer,
                // but wouldn't know that the end has to be skipped.
                b.skip.store(collapsed_tail, Ordering::Release);
                // TODO: make this a reusable function?
                tail = b.increment(tail, b.capacity() - collapsed_tail);
            }
            tail = b.increment(tail, n);
            b.tail.store(tail, Ordering::Release);
            self.producer.cached_tail.set(tail);
            n
        }
    };
    (bip = no) => {
        unsafe fn commit_unchecked(self, n: usize) -> usize {
            let p = self.producer;
            let tail = p.buffer.increment(p.cached_tail.get(), n);
            p.buffer.tail.store(tail, Ordering::Release);
            p.cached_tail.set(tail);
            n
        }
    };
}

macro_rules! fn_read_chunk_uninit_commit_unchecked {
    (contiguous = yes) => {
        unsafe fn commit_unchecked(self, n: usize) -> usize {
            for i in 0..n {
                // SAFETY: The caller must make sure that there are n initialized elements.
                unsafe { self.ptr.add(i).drop_in_place() };
            }
            let b = &self.consumer.buffer;
            let head = b.increment(self.consumer.cached_head.get(), n);
            b.head.store(head, Ordering::Release);
            self.consumer.cached_head.set(head);
            n
        }
    };
    (contiguous = no) => {
        unsafe fn commit_unchecked(self, n: usize) -> usize {
            let first_len = self.first_len.min(n);
            for i in 0..first_len {
                // SAFETY: The caller must make sure that there are n initialized elements.
                unsafe { self.first_ptr.add(i).drop_in_place() };
            }
            let second_len = self.second_len.min(n - first_len);
            for i in 0..second_len {
                // SAFETY: The caller must make sure that there are n initialized elements.
                unsafe { self.second_ptr.add(i).drop_in_place() };
            }
            let c = self.consumer;
            let head = c.buffer.increment(c.cached_head.get(), n);
            c.buffer.head.store(head, Ordering::Release);
            c.cached_head.set(head);
            n
        }
    };
}

macro_rules! fn_producer_write_chunk_uninit {
    (array = $array:ident, bip = yes) => {
        pub fn write_chunk_uninit(
            &mut self,
            n: usize,
) -> Result<ty!(WriteChunkUninit, array = $array, '_), ChunkError> {
            let b = &self.buffer;
            let mut head = self.cached_head.get();
            let tail = self.cached_tail.get();
            // TODO: check if everything is compatible with power-of-2 addressing.
            let mut slots = 0;
            let mut head_has_been_refreshed = false;
            // Collapsing the indices makes it impossible to distinguish empty and full,
            // so we check for emptiness before collapsing.
            let is_empty = head == tail;
            let mut collapsed_head = b.collapse_position(head);
            let collapsed_tail = b.collapse_position(tail);
            if !is_empty && collapsed_tail <= collapsed_head {
                // Is there enough space between `tail` and `head`?
                slots = collapsed_head - collapsed_tail;
                if slots < n {
                    // Refresh head ...
                    head = b.head.load(Ordering::Acquire);
                    self.cached_head.set(head);
                    collapsed_head = b.collapse_position(head);
                    head_has_been_refreshed = true;
                    // ... and try again.
                    let is_empty = head == tail;
                    if !is_empty && collapsed_tail <= collapsed_head {
                        // `head` did not wrap around.
                        slots = collapsed_head - collapsed_tail;
                        if slots < n {
                            return Err(ChunkError::TooFewSlots(slots));
                        }
                    } else {
                        // `head` did wrap around, we'll continue below.
                    }
                }
            } else {
                // No need to refresh `head`, it cannot overtake `tail`.
            }
            let offset;
            if slots < n {
                // Is there enough space at the end of the buffer?
                slots = b.capacity() - collapsed_tail;
                if slots < n {
                    // Nope, let's check the beginning.

                    // TODO: interaction/reuse with slots() et al.?

                    slots = slots.max(collapsed_head);
                    if slots < n {
                        // TODO: check if this early return/local variable is an actual optimization?
                        if head_has_been_refreshed {
                            return Err(ChunkError::TooFewSlots(slots));
                        }
                        head = b.head.load(Ordering::Acquire);
                        self.cached_head.set(head);
                        collapsed_head = b.collapse_position(head);
                        slots = slots.max(collapsed_head);
                        if slots < n {
                            return Err(ChunkError::TooFewSlots(slots));
                        }
                    }
                    // NB: `tail` will be (conditionally) reset in `commit_unchecked()`.
                    offset = 0;
                } else {
                    offset = collapsed_tail;
                }
            } else {
                offset = collapsed_tail;
            }
            // SAFETY: `offset` has been set to a valid position.
            Ok(unsafe { WriteChunkUninit::new(self, n, offset) })
        }
    };
    (array = $array:ident, bip = no) => {
        pub fn write_chunk_uninit(
            &mut self,
            n: usize,
) -> Result<ty!(WriteChunkUninit, array = $array, '_), ChunkError> {
            let head = self.cached_head.get();
            let tail = self.cached_tail.get();
            let b = &self.buffer;
            // Check if the queue has *possibly* not enough slots.
            if b.capacity() - b.distance(head, tail) < n {
                // Refresh the head ...
                let head = b.head.load(Ordering::Acquire);
                self.cached_head.set(head);
                // ... and check if there *really* are not enough slots.
                let slots = b.capacity() - b.distance(head, tail);
                if slots < n {
                    return Err(ChunkError::TooFewSlots(slots));
                }
            }
            let offset = b.collapse_position(tail);
            // SAFETY: `offset` has been set to a valid position.
            Ok(unsafe { WriteChunkUninit::new(self, n, offset) })
        }
    };
}

macro_rules! fn_producer_write_chunk {
    (array = $array:ident, contiguous = $contiguous:ident) => {
        /// Returns `n` slots (initially containing their [`Default`] value) for writing.
        ///
        #[doc = choice!($contiguous, "\
        [`WriteChunk::as_mut_slice()`]", "\
        [`WriteChunk::as_mut_slices()`]")]
        /// provides mutable access to the slots.
        /// After writing to those slots, they explicitly have to be made available
        /// to be read by the [`Consumer`] by calling [`WriteChunk::commit()`]
        /// or [`WriteChunk::commit_all()`].
        ///
        /// For an alternative that does not require the trait bound [`Default`],
        /// see [`Producer::write_chunk_uninit()`].
        ///
        /// If items are supposed to be moved from an iterator into the ring buffer,
        /// [`Producer::write_chunk_uninit()`] followed by [`WriteChunkUninit::fill_from_iter()`]
        /// can be used.
        ///
        /// # Errors
        ///
        /// If not enough slots are available, an error
        /// (containing the number of available slots) is returned.
        /// Use [`Producer::slots()`] to obtain the number of available slots beforehand.
        ///
        /// TODO: mention different types of slots...() for bip?
        ///
        /// # Examples
        ///
        /// See the documentation of the [`chunks`](crate::chunks#examples) module.
        pub fn write_chunk(
            &mut self,
            n: usize,
) -> Result<ty!(WriteChunk, array = $array, '_), ChunkError>
        where
            T: Default,
        {
            self.write_chunk_uninit(n).map(WriteChunk::from)
        }
    };
}

macro_rules! fn_consumer_read_chunk {
    (array = $array:ident, bip = yes) => {
        pub fn read_chunk(
            &mut self,
            n: usize,
) -> Result<ty!(ReadChunk, array = $array, '_), ChunkError> {
            let b = &self.buffer;
            let mut head = self.cached_head.get();
            let mut tail = self.cached_tail.get();
            let mut slots = 0;
            let mut tail_has_been_refreshed = false;
            // Collapsing the indices makes it impossible to distinguish empty and full,
            // so we check for emptiness before collapsing.
            let is_empty = head == tail;
            let mut collapsed_head = b.collapse_position(head);
            let mut collapsed_tail = b.collapse_position(tail);
            if is_empty || collapsed_head < collapsed_tail {
                slots = collapsed_tail - collapsed_head;
                if slots < n {
                    // Refresh the tail ...
                    tail = b.tail.load(Ordering::Acquire);
                    tail_has_been_refreshed = true;
                    self.cached_tail.set(tail);
                    collapsed_tail = b.collapse_position(tail);
                    // ... and check again.
                    let is_empty = head == tail;
                    if is_empty || collapsed_head < collapsed_tail {
                        // `tail` did not wrap around.
                        slots = collapsed_tail - collapsed_head;
                        if slots < n {
                            return Err(ChunkError::TooFewSlots(slots));
                        }
                    } else {
                        // `tail` did wrap around, we'll continue below.
                    }
                }
            } else {
                // No need to refresh `tail`, it cannot overtake `head`.
            }
            if slots < n {
                // NB: We are only allowed to use `skip` if (collapsed) `tail < head`
                //     (or if the buffer is full).
                let skip = b.skip.load(Ordering::Acquire);
                slots = skip - collapsed_head;
                if slots == 0 {
                    // No more slots at the end of the buffer, let's wrap around.
                    if skip != b.capacity() {
                        b.skip.store(b.capacity(), Ordering::Release);
                    }
                    head = b.increment(head, b.capacity() - collapsed_head);
                    // NB: `skip` is stored before `head`.
                    b.head.store(head, Ordering::Release);
                    self.cached_head.set(head);
                    collapsed_head = b.collapse_position(head);
                    slots = collapsed_tail - collapsed_head;
                    if slots < n {
                        if tail_has_been_refreshed {
                            return Err(ChunkError::TooFewSlots(slots));
                        }
                        tail = b.tail.load(Ordering::Acquire);
                        self.cached_tail.set(tail);
                        collapsed_tail = b.collapse_position(tail);
                        slots = collapsed_tail - collapsed_head;
                    }
                }
                if slots < n {
                    return Err(ChunkError::TooFewSlots(slots));
                }
            }
            let offset = collapsed_head;
            // SAFETY: `offset` has been set to a valid position.
            Ok(unsafe { ReadChunk::new(self, n, offset) })
        }
    };
    (array = $array:ident, bip = no) => {
        pub fn read_chunk(
            &mut self,
            n: usize,
) -> Result<ty!(ReadChunk, array = $array, '_), ChunkError> {
            let head = self.cached_head.get();
            let tail = self.cached_tail.get();
            let b = &self.buffer;
            // Check if the queue has *possibly* not enough slots.
            if b.distance(head, tail) < n {
                // Refresh the tail ...
                let tail = b.tail.load(Ordering::Acquire);
                self.cached_tail.set(tail);
                // ... and check if there *really* are not enough slots.
                let slots = b.distance(head, tail);
                if slots < n {
                    return Err(ChunkError::TooFewSlots(slots));
                }
            }
            let offset = b.collapse_position(head);
            // SAFETY: `offset` has been set to a valid position.
            Ok(unsafe { ReadChunk::new(self, n, offset) })
        }
    };
}

macro_rules! fn_write_chunk_uninit_as_mut_sliceX_docstring {
    () => { docstring!(
        ///
        /// After writing to the slots, they are *not* automatically made available
        /// to be read by the [`Consumer`].
        /// This has to be explicitly done by calling [`commit()`](WriteChunkUninit::commit)
        /// or [`commit_all()`](WriteChunkUninit::commit_all).
        /// If items are written but *not* committed afterwards,
        /// they will *not* become available for reading and
        /// they will be leaked (which is only relevant if `T` implements [`Drop`]).
    )};
}

macro_rules! fn_write_chunk_uninit_as_mut_sliceX {
    (contiguous = yes) => {
        /// Returns a slice for writing to the requested slots.
        ///
        /// The extension trait [`CopyToUninit`] can be used
        /// to safely copy data into this slice.
        #[doc = fn_write_chunk_uninit_as_mut_sliceX_docstring!()]
        pub fn as_mut_slice(&mut self) -> &mut [MaybeUninit<T>] {
            // SAFETY: The pointer and length have been computed correctly in write_chunk_uninit().
            unsafe { core::slice::from_raw_parts_mut(self.ptr.cast(), self.len) }
        }
    };
    (contiguous = no) => {
        /// Returns two slices for writing to the requested slots.
        ///
        /// The first slice can only be empty if `0` slots have been requested.
        /// If the first slice contains all requested slots, the second one is empty.
        ///
        /// The extension trait [`CopyToUninit`](crate::CopyToUninit) can be used
        /// to safely copy data into those slices.
        #[doc = fn_write_chunk_uninit_as_mut_sliceX_docstring!()]
        pub fn as_mut_slices(&mut self) -> (&mut [MaybeUninit<T>], &mut [MaybeUninit<T>]) {
            // SAFETY: The pointers and lengths have been computed correctly in write_chunk_uninit().
            unsafe {
                (
                    core::slice::from_raw_parts_mut(self.first_ptr.cast(), self.first_len),
                    core::slice::from_raw_parts_mut(self.second_ptr.cast(), self.second_len),
                )
            }
        }
    };
}

/// Drops all elements starting from index `n`.
///
/// # Safety
///
/// All of those slots must be initialized.
macro_rules! fn_write_chunk_uninit_drop_suffix {
    (contiguous = yes) => {
        unsafe fn drop_suffix(&mut self, n: usize) {
            // NB: If n >= self.len(), the loop is not entered.
            for i in n..self.len {
                // SAFETY: The caller must make sure that all slots are initialized.
                unsafe { self.ptr.add(i).drop_in_place() };
            }
        }
    };
    (contiguous = no) => {
        unsafe fn drop_suffix(&mut self, n: usize) {
            // NB: If n >= self.len(), the loops are not entered.
            for i in n..self.first_len {
                // SAFETY: The caller must make sure that all slots are initialized.
                unsafe { self.first_ptr.add(i).drop_in_place() };
            }
            for i in n.saturating_sub(self.first_len)..self.second_len {
                // SAFETY: The caller must make sure that all slots are initialized.
                unsafe { self.second_ptr.add(i).drop_in_place() };
            }
        }
    };
}

macro_rules! fn_write_chunk_as_mut_sliceX_docstring {
    () => { docstring!(
        ///
        /// After writing to the slots, they are *not* automatically made available
        /// to be read by the [`Consumer`].
        /// This has to be explicitly done by calling [`commit()`](WriteChunk::commit)
        /// or [`commit_all()`](WriteChunk::commit_all).
        /// If items are written but *not* committed afterwards,
        /// they will *not* become available for reading and
        /// they will eventually be dropped (if `T` implements [`Drop`]).
    )};
}

macro_rules! fn_write_chunk_as_mut_sliceX {
    (contiguous = yes) => {
        /// Returns a slice for writing to the requested slots.
        ///
        /// All slots are initially filled with their [`Default`] value.
        #[doc = fn_write_chunk_as_mut_sliceX_docstring!()]
        pub fn as_mut_slice(&mut self) -> &mut [T] {
            // self.0 is always Some(chunk).
            let chunk = self.0.as_ref().unwrap();
            // SAFETY: The pointer and length have been computed correctly in write_chunk_uninit()
            // and all slots have been initialized in From::from().
            unsafe { core::slice::from_raw_parts_mut(chunk.ptr, chunk.len) }
        }
    };
    (contiguous = no) => {
        /// Returns two slices for writing to the requested slots.
        ///
        /// All slots are initially filled with their [`Default`] value.
        ///
        /// The first slice can only be empty if `0` slots have been requested.
        /// If the first slice contains all requested slots, the second one is empty.
        #[doc = fn_write_chunk_as_mut_sliceX_docstring!()]
        pub fn as_mut_slices(&mut self) -> (&mut [T], &mut [T]) {
            // self.0 is always Some(chunk).
            let chunk = self.0.as_ref().unwrap();
            // SAFETY: The pointers and lengths have been computed correctly in write_chunk_uninit()
            // and all slots have been initialized in From::from().
            unsafe {
                (
                    core::slice::from_raw_parts_mut(chunk.first_ptr, chunk.first_len),
                    core::slice::from_raw_parts_mut(chunk.second_ptr, chunk.second_len),
                )
            }
        }
    };
}

macro_rules! fn_read_chunk_as_sliceX_docstring {
    () => { docstring!(
        ///
        /// The provided slots are *not* automatically made available
        /// to be written again by the [`Producer`].
        /// This has to be explicitly done by calling [`commit()`](ReadChunk::commit)
        /// or [`commit_all()`](ReadChunk::commit_all).
        /// Note that this runs the destructor of the committed items (if `T` implements [`Drop`]).
        /// You can "peek" at the contained values by simply not calling any of the "commit" methods.
    )};
}

macro_rules! fn_read_chunk_as_sliceX {
    (contiguous = yes) => {
        /// Returns a slice for reading from the requested slots.
        #[doc = fn_read_chunk_as_sliceX_docstring!()]
        pub fn as_slice(&self) -> &[T] {
            // SAFETY: The correct pointer and length have been provided by ReadChunk::new().
            unsafe { core::slice::from_raw_parts(self.ptr, self.len) }
        }
    };
    (contiguous = no) => {
        /// Returns two slices for reading from the requested slots.
        ///
        /// The first slice can only be empty if `0` slots have been requested.
        /// If the first slice contains all requested slots, the second one is empty.
        #[doc = fn_read_chunk_as_sliceX_docstring!()]
        pub fn as_slices(&self) -> (&[T], &[T]) {
            // SAFETY: The pointers and lengths have been computed correctly in read_chunk().
            unsafe {
                (
                    core::slice::from_raw_parts(self.first_ptr, self.first_len),
                    core::slice::from_raw_parts(self.second_ptr, self.second_len),
                )
            }
        }
    };
}

macro_rules! fn_read_chunk_as_mut_sliceX_docstring {
    () => { docstring!(
        ///
        /// In the vast majority of cases, mutable access is not required when
        /// reading data and the immutable version should be preferred. However,
        /// there are some scenarios where it might be desirable to perform
        /// operations on the data in-place without copying it to a separate buffer
        /// (e.g. streaming decryption), in which case this version can be used.
    )};
}

macro_rules! fn_read_chunk_as_mut_sliceX {
    (contiguous = yes) => {
        /// Returns a mutable slice for reading from the requested slots.
        ///
        /// This has the same semantics as [`as_slice()`](ReadChunk::as_slice),
        /// except that it returns a mutable slice and requires a mutable reference
        /// to the chunk.
        #[doc = fn_read_chunk_as_mut_sliceX_docstring!()]
        pub fn as_mut_slice(&mut self) -> &mut [T] {
            // SAFETY: The correct pointer and length have been provided by ReadChunk::new().
            unsafe { core::slice::from_raw_parts_mut(self.ptr, self.len) }
        }
    };
    (contiguous = no) => {
        /// Returns two mutable slices for reading from the requested slots.
        ///
        /// This has the same semantics as [`as_slices()`](ReadChunk::as_slices),
        /// except that it returns mutable slices and requires a mutable reference
        /// to the chunk.
        #[doc = fn_read_chunk_as_mut_sliceX_docstring!()]
        pub fn as_mut_slices(&mut self) -> (&mut [T], &mut [T]) {
            // SAFETY: The pointers and lengths have been computed correctly in read_chunk().
            unsafe {
                (
                    core::slice::from_raw_parts_mut(self.first_ptr, self.first_len),
                    core::slice::from_raw_parts_mut(self.second_ptr, self.second_len),
                )
            }
        }
    };
}

macro_rules! fn_X_chunk_X_len_and_is_empty {
    (contiguous = yes) => {
        /// Returns the number of slots in the chunk.
        pub fn len(&self) -> usize {
            self.len
        }

        /// Returns `true` if the chunk contains no slots.
        pub fn is_empty(&self) -> bool {
            self.len == 0
        }
    };
    (contiguous = no) => {
        /// Returns the number of slots in the chunk.
        pub fn len(&self) -> usize {
            self.first_len + self.second_len
        }

        /// Returns `true` if the chunk contains no slots.
        pub fn is_empty(&self) -> bool {
            self.first_len == 0
        }
    };
}

macro_rules! fn_write_chunk_len_and_is_empty {
    () => {
        /// Returns the number of slots in the chunk.
        pub fn len(&self) -> usize {
            // self.0 is always Some(chunk).
            self.0.as_ref().unwrap().len()
        }

        /// Returns `true` if the chunk contains no slots.
        pub fn is_empty(&self) -> bool {
            // self.0 is always Some(chunk).
            self.0.as_ref().unwrap().is_empty()
        }
    };
}

macro_rules! struct_write_chunk_uninit {
    (arc = $arc:ident, array = $array:ident, contiguous = yes) => {
        // TODO: implement manually:
        //#[derive(Debug, PartialEq, Eq)]
        struct_!(WriteChunkUninit, array = $array, params = ('a, T), fields = {
            ptr: *mut T,
            len: usize,
            producer: &'a ty!(Producer, arc = $arc, array = $array),
        });

        impl_!(WriteChunkUninit<'a>, array = $array, {
            unsafe fn new(producer: &'a ty!(Producer, arc = $arc, array = $array), n: usize, offset: usize) -> Self {
                Self {
                    // SAFETY: Caller must guarantee that `offset` is valid.
                    ptr: unsafe { producer.buffer.data_ptr().add(offset) },
                    len: n,
                    producer,
                }
            }
        });

        impl_!(WriteChunk<'a>, trait = From<ty!(WriteChunkUninit, array = $array, 'a)>, array = $array,
        where
            T: Default,
        {
            /// Fills all slots with the [`Default`] value.
            fn from(chunk: ty!(WriteChunkUninit, array = $array, 'a)) -> Self {
                for i in 0..chunk.len {
                    // SAFETY: i is in a valid range.
                    unsafe { chunk.ptr.add(i).write(Default::default()) };
                }
                WriteChunk(Some(chunk))
            }
        });
    };
    (arc = $arc:ident, array = $array:ident, contiguous = no) => {
        // TODO: implement manually:
        //#[derive(Debug, PartialEq, Eq)]
        struct_!(WriteChunkUninit, array = $array, params = ('a, T), fields = {
            first_ptr: *mut T,
            first_len: usize,
            second_ptr: *mut T,
            second_len: usize,
            producer: &'a ty!(Producer, arc = $arc, array = $array),
        });

        impl_!(WriteChunkUninit<'a>, array = $array, {
            unsafe fn new(producer: &'a ty!(Producer, arc = $arc, array = $array), n: usize, offset: usize) -> Self {
                let first_len = n.min(producer.buffer.capacity() - offset);
                Self {
                    // SAFETY: Caller must guarantee that `offset` is valid.
                    first_ptr: unsafe { producer.buffer.data_ptr().add(offset) },
                    first_len,
                    second_ptr: producer.buffer.data_ptr(),
                    second_len: n - first_len,
                    producer,
                }
            }
        });

        impl_!(WriteChunk<'a>, trait = From<ty!(WriteChunkUninit, array = $array, 'a)>, array = $array,
        where
            T: Default,
        {
            /// Fills all slots with the [`Default`] value.
            fn from(chunk: ty!(WriteChunkUninit, array = $array, 'a)) -> Self {
                for i in 0..chunk.first_len {
                    // SAFETY: i is in a valid range.
                    unsafe { chunk.first_ptr.add(i).write(Default::default()) };
                }
                for i in 0..chunk.second_len {
                    // SAFETY: i is in a valid range.
                    unsafe { chunk.second_ptr.add(i).write(Default::default()) };
                }
                WriteChunk(Some(chunk))
            }
        });
    };
}

macro_rules! struct_write_chunk {
    (array = $array:ident) => {
        //#[derive(Debug, PartialEq, Eq)]
        struct_!(WriteChunk, array = $array, params = ('a, T), fields = (Option<ty!(WriteChunkUninit, array = $array, 'a)>););

        impl_!(WriteChunk<'_>, trait = Drop, array = $array, {
            fn drop(&mut self) {
                // NB: If `commit()` or `commit_all()` has been called, `self.0` is `None`.
                if let Some(mut chunk) = self.0.take() {
                    // No part of the chunk has been committed, all slots are dropped.
                    // SAFETY: All slots have been initialized in From::from().
                    unsafe { chunk.drop_suffix(0) };
                }
            }
        });
    };
}

macro_rules! struct_read_chunk {
    (arc = $arc:ident, array = $array:ident, contiguous = yes) => {
        // TODO: implement manually:
        //#[derive(Debug, PartialEq, Eq)]
        struct_!(ReadChunk, array = $array, params = ('a, T), fields = {
            ptr: *mut T,
            len: usize,
            consumer: &'a ty!(Consumer, arc = $arc, array = $array),
        });

        impl_!(ReadChunk<'a>, array = $array, {
            unsafe fn new(consumer: &'a ty!(Consumer, arc = $arc, array = $array), n: usize, offset: usize) -> Self {
                Self {
                    // SAFETY: Caller must guarantee that `offset` is valid.
                    ptr: unsafe { consumer.buffer.data_ptr().add(offset) },
                    len: n,
                    consumer,
                }
            }
        });
    };
    (arc = $arc:ident, array = $array:ident, contiguous = no) => {
        // TODO: implement manually:
        //#[derive(Debug, PartialEq, Eq)]
        struct_!(ReadChunk, array = $array, params = ('a, T), fields = {
            // Must be "mut" for drop_in_place()
            first_ptr: *mut T,
            first_len: usize,
            // Must be "mut" for drop_in_place()
            second_ptr: *mut T,
            second_len: usize,
            consumer: &'a ty!(Consumer, arc = $arc, array = $array),
        });

        impl_!(ReadChunk<'a>, array = $array, {
            unsafe fn new(consumer: &'a ty!(Consumer, arc = $arc, array = $array), n: usize, offset: usize) -> Self {
                let b = &consumer.buffer;
                let first_len = n.min(b.capacity() - offset);
                Self {
                    // SAFETY: Caller must guarantee that `offset` is valid.
                    first_ptr: unsafe { b.data_ptr().add(offset) },
                    first_len,
                    second_ptr: b.data_ptr(),
                    second_len: n - first_len,
                    consumer,
                }
            }
        });
    };
}

macro_rules! fn_write_chunk_uninit_commit_all {
    () => {
        /// Makes the whole chunk available for reading.
        ///
        /// # Safety
        ///
        /// The caller must make sure that all elements have been initialized.
        pub unsafe fn commit_all(self) {
            let slots = self.len();
            // SAFETY: Delegated to the caller.
            unsafe { self.commit_unchecked(slots) };
        }
    };
}

macro_rules! fn_write_chunk_uninit_commit {
    () => {
        /// Makes the first `n` slots of the chunk available for reading.
        ///
        /// # Panics
        ///
        /// Panics if `n` is greater than the number of slots in the chunk.
        ///
        /// # Safety
        ///
        /// The caller must make sure that the first `n` elements have been initialized.
        pub unsafe fn commit(self, n: usize) {
            assert!(n <= self.len(), "cannot commit more than chunk size");
            // SAFETY: Delegated to the caller.
            unsafe { self.commit_unchecked(n) };
        }
    };
}

macro_rules! fn_write_chunk_uninit_fill_from_iter_docstring {
    (arc = $arc:ident, array = $array:ident, module = $module:literal) => { docstring!(
        /// Moves items from an iterator into the (uninitialized) slots of the chunk.
        ///
        /// The number of moved items is returned.
        ///
        /// All moved items are automatically made availabe to be read by the [`Consumer`].
        ///
        /// # Examples
        ///
        /// If the iterator contains too few items, only a part of the chunk
        /// is made available for reading:
        ///
        /// ```
        #[doc = doctest_import!($module, "{PopError, RingBuffer}")]
        ///
        #[doc = doctest_create_ring_buffer!(arc = $arc, array = $array, capacity = 4)]
        /// if let Ok(chunk) = p.write_chunk_uninit(3) {
        ///     assert_eq!(chunk.fill_from_iter([10, 20]), 2);
        /// } else {
        ///     unreachable!();
        /// }
        /// assert_eq!(p.slots(), 2);
        /// assert_eq!(c.pop(), Ok(10));
        /// assert_eq!(c.pop(), Ok(20));
        /// assert_eq!(c.pop(), Err(PopError::Empty));
        /// ```
        ///
        /// If the chunk size is too small, some items may remain in the iterator.
        /// To be able to keep using the iterator after the call,
        /// `&mut` (or [`Iterator::by_ref()`]) can be used.
        ///
        /// ```
        #[doc = doctest_import!($module, "{PopError, RingBuffer}")]
        ///
        #[doc = doctest_create_ring_buffer!(arc = $arc, array = $array, capacity = 4)]
        /// let mut it = vec![10, 20, 30].into_iter();
        /// if let Ok(chunk) = p.write_chunk_uninit(2) {
        ///     assert_eq!(chunk.fill_from_iter(&mut it), 2);
        /// } else {
        ///     unreachable!();
        /// }
        /// assert_eq!(c.pop(), Ok(10));
        /// assert_eq!(c.pop(), Ok(20));
        /// assert_eq!(c.pop(), Err(PopError::Empty));
        /// assert_eq!(it.next(), Some(30));
        /// ```
    )}
}

macro_rules! fn_write_chunk_uninit_fill_from_iter {
    (arc = $arc:ident, array = $array:ident, contiguous = yes, module = $module:literal) => {
        #[doc = fn_write_chunk_uninit_fill_from_iter_docstring!(arc = $arc, array = $array, module = $module)]
        pub fn fill_from_iter<I>(self, iter: I) -> usize
        where
            I: IntoIterator<Item = T>,
        {
            let mut iter = iter.into_iter();
            let mut iterated = 0;
            for i in 0..self.len {
                match iter.next() {
                    Some(item) => {
                        // SAFETY: It is allowed to write to this memory slot
                        unsafe { self.ptr.add(i).write(item) };
                        iterated += 1;
                    }
                    None => break,
                }
            }
            // SAFETY: iterated slots have been initialized above
            unsafe { self.commit_unchecked(iterated) }
        }
    };
    (arc = $arc:ident, array = $array:ident, contiguous = no, module = $module:literal) => {
        #[doc = fn_write_chunk_uninit_fill_from_iter_docstring!(arc = $arc, array = $array, module = $module)]
        pub fn fill_from_iter<I>(self, iter: I) -> usize
        where
            I: IntoIterator<Item = T>,
        {
            let mut iter = iter.into_iter();
            let mut iterated = 0;
            'outer: for &(ptr, len) in &[
                (self.first_ptr, self.first_len),
                (self.second_ptr, self.second_len),
            ] {
                for i in 0..len {
                    match iter.next() {
                        Some(item) => {
                            // SAFETY: It is allowed to write to this memory slot
                            unsafe { ptr.add(i).write(item) };
                            iterated += 1;
                        }
                        None => break 'outer,
                    }
                }
            }
            // SAFETY: iterated slots have been initialized above
            unsafe { self.commit_unchecked(iterated) }
        }
    };
}
macro_rules! fn_write_chunk_commit_all {
    () => {
        /// Makes the whole chunk available for reading.
        pub fn commit_all(mut self) {
            // self.0 is always Some(chunk).
            let chunk = self.0.take().unwrap();
            // SAFETY: All slots have been initialized in From::from().
            unsafe { chunk.commit_all() };
            // `self` is dropped here, with `self.0` being set to `None`.
        }
    };
}

macro_rules! fn_write_chunk_commit {
    () => {
        /// Makes the first `n` slots of the chunk available for reading.
        ///
        /// The rest of the chunk is dropped.
        ///
        /// # Panics
        ///
        /// Panics if `n` is greater than the number of slots in the chunk.
        pub fn commit(mut self, n: usize) {
            // self.0 is always Some(chunk).
            let mut chunk = self.0.take().unwrap();
            // SAFETY: All slots have been initialized in From::from().
            unsafe {
                // Slots at index `n` and higher are dropped ...
                chunk.drop_suffix(n);
                // ... everything below `n` is committed.
                chunk.commit(n);
            }
            // `self` is dropped here, with `self.0` being set to `None`.
        }
    };
}

macro_rules! fn_read_chunk_commit_all {
    () => {
        /// Drops all slots of the chunk, making the space available for writing again.
        pub fn commit_all(self) {
            let slots = self.len();
            // SAFETY: self.len() initialized elements have been obtained in read_chunk().
            unsafe { self.commit_unchecked(slots) };
        }
    };
}

macro_rules! fn_read_chunk_commit {
    (arc = $arc:ident, array = $array:ident, module = $module:literal) => {
        /// Drops the first `n` slots of the chunk, making the space available for writing again.
        ///
        /// # Panics
        ///
        /// Panics if `n` is greater than the number of slots in the chunk.
        ///
        /// # Examples
        ///
        /// The following example shows that items are dropped when "committed"
        /// (which is only relevant if `T` implements [`Drop`]).
        ///
        /// ```
        #[doc = doctest_import!($module, "RingBuffer")]
        ///
        /// // Static variable to count all drop() invocations
        /// static mut DROP_COUNT: i32 = 0;
        /// #[derive(Debug)]
        /// struct Thing;
        /// impl Drop for Thing {
        ///     fn drop(&mut self) { unsafe { DROP_COUNT += 1; } }
        /// }
        ///
        /// // Scope to limit lifetime of ring buffer
        /// {
        #[doc = doctest_create_ring_buffer!(arc = $arc, array = $array, capacity = 4, "    ")]
        ///
        ///     assert!(p.push(Thing).is_ok()); // 1
        ///     assert!(p.push(Thing).is_ok()); // 2
        ///     if let Ok(thing) = c.pop() {
        ///         // "thing" has been *moved* out of the queue but not yet dropped
        ///         assert_eq!(unsafe { DROP_COUNT }, 0);
        ///     } else {
        ///         unreachable!();
        ///     }
        ///     // First Thing has been dropped when "thing" went out of scope:
        ///     assert_eq!(unsafe { DROP_COUNT }, 1);
        ///     assert!(p.push(Thing).is_ok()); // 3
        ///
        ///     if let Ok(chunk) = c.read_chunk(2) {
        ///         assert_eq!(chunk.len(), 2);
        ///         assert_eq!(unsafe { DROP_COUNT }, 1);
        ///         chunk.commit(1); // Drops only one of the two Things
        ///         assert_eq!(unsafe { DROP_COUNT }, 2);
        ///     } else {
        ///         unreachable!();
        ///     }
        ///     // The last Thing is still in the queue ...
        ///     assert_eq!(unsafe { DROP_COUNT }, 2);
        /// }
        /// // ... and it is dropped when the ring buffer goes out of scope:
        /// assert_eq!(unsafe { DROP_COUNT }, 3);
        /// ```
        pub fn commit(self, n: usize) {
            assert!(n <= self.len(), "cannot commit more than chunk size");
            // SAFETY: self.len() initialized elements have been obtained in read_chunk().
            unsafe { self.commit_unchecked(n) };
        }
    };
}

macro_rules! impl_send_for_chunks {
    (array = $array:ident, module = $module:literal) => {
        // SAFETY: WriteChunkUninit only exists while a unique reference to the producer is held.
        // It is therefore safe to move it to another thread.
        unsafe_impl!(
            /// It (as well as [`WriteChunk`]) can be moved ...
            /// ```
            #[doc = doctest_import!($module, "{WriteChunk, WriteChunkUninit}")]
            /// fn assert_send<X: Send>() {}
            #[doc = concat!("assert_send::<", doctest_ty!(WriteChunkUninit, u8, 8, array = $array), ">();")]
            #[doc = concat!("assert_send::<", doctest_ty!(WriteChunk, u8, 8, array = $array), ">();")]
            /// ```
            /// ... but not shared between threads:
            /// ```compile_fail
            #[doc = doctest_import!($module, "WriteChunkUninit", "# ")]
            /// fn assert_sync<X: Sync>() {}
            #[doc = concat!("assert_sync::<", doctest_ty!(WriteChunkUninit, u8, 8, array = $array), ">();")]
            /// ```
            /// ```compile_fail
            #[doc = doctest_import!($module, "WriteChunk", "# ")]
            /// # fn assert_sync<X: Sync>() {}
            #[doc = concat!("assert_sync::<", doctest_ty!(WriteChunk, u8, 8, array = $array), ">();")]
            /// ```
            Send, for = ty!(WriteChunkUninit, array = $array, '_), array = $array, where T: Send);

        // SAFETY: ReadChunk only exists while a unique reference to the consumer is held.
        // It is therefore safe to move it to another thread.
        unsafe_impl!(
            /// It (and any wrapper structs) can be moved ...
            /// ```
            #[doc = doctest_import!($module, "ReadChunk")]
            /// fn assert_send<X: Send>() {}
            #[doc = concat!("assert_send::<", doctest_ty!(ReadChunk, u8, 8, array = $array), ">();")]
            /// ```
            /// ... but not shared between threads:
            /// ```compile_fail
            #[doc = doctest_import!($module, "ReadChunk", "# ")]
            /// fn assert_sync<X: Sync>() {}
            #[doc = concat!("assert_sync::<", doctest_ty!(ReadChunk, u8, 8, array = $array), ">();")]
            /// ```
            Send, for = ty!(ReadChunk, array = $array, '_), array = $array, where T: Send);
    };
}
