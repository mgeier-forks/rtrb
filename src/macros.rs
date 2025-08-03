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
        use crate::atomic::*;
        use crate::CachePadded;
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
            #[allow(clippy::new_ret_no_self)]
            pub fn new(capacity: usize) -> (Producer<T>, Consumer<T>) {
                let capacity = Self::update_capacity(capacity);
                ArcRingBuffer::new(Self {
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
    (padded = $padded:ident, bip = yes, rb_doc = $rb_doc:expr) => {
        storage_array_helper!(padded = $padded, skip, rb_doc = $rb_doc);
    };
    (padded = $padded:ident, bip = no, rb_doc = $rb_doc:expr) => {
        storage_array_helper!(padded = $padded, , rb_doc = $rb_doc);
    };
}

macro_rules! storage_array_helper {
    (padded = $padded:ident, $($skip:ident)?, rb_doc = $rb_doc:expr) => {
        use crate::atomic::*;
        use crate::cache_padded::CachePadded;
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
            pub const fn new() -> Self {
                const {
                    assert!(
                        Self::update_capacity(N) == N,
                        "`capacity` must be a power of two"
                    );
                }
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
        bip = $bip:ident,
        contiguous = $contiguous:ident,
        pow2 = $pow2:ident,
        'a = ($($a:lifetime)?),
        N = ($($N:ident)?)
    ) => {
        check_bip_contiguous!($bip, $contiguous);

        use core::cell::Cell;
        use core::mem::MaybeUninit;

        // TODO: move definition here?
        #[allow(unused_imports)]
        use crate::diy::IS_ABANDONED;
        #[allow(unused_imports)]
        use crate::diy::{HAS_CONSUMER, HAS_PRODUCER};

        // TODO: move error type to top level?
        use crate::chunks::ChunkError;
        use crate::{PeekError, PopError, PushError};

        // SAFETY: RingBuffer is only mutated (using *interior mutablility*)
        // via Producer/Consumer (which are !Sync), all other access can be shared.
        unsafe impl<T: Send$(, const $N: usize)?> Sync for RingBuffer<T$(, $N)?> {}

        impl<T$(, const $N: usize)?> RingBuffer<T$(, $N)?> {
            unsafe fn slot_ptr(&self, pos: usize) -> *mut T {
                // SAFETY: See docstring.
                unsafe { self.data_ptr().add(self.collapse_position(pos)) }
            }
            fn_ring_buffer_producer!(arc = $arc, Producer<T$(, $N)?>);
            fn_ring_buffer_consumer!(arc = $arc, Consumer<T$(, $N)?>);
            fn_ring_buffer_drop_all_elements!(bip = $bip);
            fn_ring_buffer_update_capacity!(pow2 = $pow2);
            fn_ring_buffer_collapse_position!(pow2 = $pow2);
            fn_ring_buffer_increment!(pow2 = $pow2);
            fn_ring_buffer_increment1!(pow2 = $pow2);
            fn_ring_buffer_distance!(pow2 = $pow2);
        }

        struct_arc_ring_buffer!(arc = $arc);

        struct_producer!(arc = $arc, N = ($($N)?));

        impl<$($a, )?T$(, const $N: usize)?> Producer<$($a, )?T$(, $N)?> {
            fn_producer_push!();
            fn_producer_write_chunk_uninit!(bip = $bip, WriteChunkUninit<'_, T$(, $N)?>);
            fn_producer_write_chunk!(contiguous = $contiguous, WriteChunk<'_, T$(, $N)?>);
            // TODO: documentation specific to bip:
            fn_producer_slots!();
            fn_producer_is_full!();
            fn_pc_capacity!();
            fn_producer_next_tail!();
        }

        struct_consumer!(arc = $arc, N = ($($N)?));

        impl<$($a, )?T$(, const $N: usize)?> Consumer<$($a, )?T$(, $N)?> {
            fn_consumer_pop!();
            fn_consumer_peek!();
            fn_consumer_read_chunk!(bip = $bip, ReadChunk<'_, T$(, $N)?>);
            // TODO: documentation specific to bip:
            fn_consumer_slots!(bip = $bip);
            fn_consumer_slots_contiguousX!(bip = $bip);
            fn_consumer_is_empty!();
            fn_pc_capacity!();
            fn_consumer_next_head!(bip = $bip);
        }

        struct_write_chunk_uninit!(contiguous = $contiguous, Producer<$($a, )?T$(, $N)?>, N = ($($N)?));
        struct_write_chunk!(N = ($($N)?));
        struct_read_chunk!(contiguous = $contiguous, Consumer<$($a, )?T$(, $N)?>, N = ($($N)?));

        impl_send_for_chunks!(N = ($($N)?));

        impl<T$(, const $N: usize)?> WriteChunkUninit<'_, T$(, $N)?> {
            fn_write_chunk_uninit_as_mut_sliceX!(contiguous = $contiguous);
            fn_write_chunk_uninit_commit_all!();
            fn_write_chunk_uninit_commit!();
            fn_write_chunk_uninit_fill_from_iter!(contiguous = $contiguous);
            fn_X_chunk_X_len_and_is_empty!(contiguous = $contiguous);

            fn_write_chunk_uninit_drop_suffix!(contiguous = $contiguous);
            fn_write_chunk_uninit_commit_unchecked!(bip = $bip);
        }

        impl<T$(, const $N: usize)?> WriteChunk<'_, T$(, $N)?> {
            fn_write_chunk_as_mut_sliceX!(contiguous = $contiguous);
            fn_write_chunk_commit_all!();
            fn_write_chunk_commit!();
            fn_write_chunk_len_and_is_empty!();
        }

        impl<T$(, const $N: usize)?> ReadChunk<'_, T$(, $N)?> {
            fn_read_chunk_as_sliceX!(contiguous = $contiguous);
            fn_read_chunk_as_mut_sliceX!(contiguous = $contiguous);
            fn_read_chunk_commit_all!();
            fn_read_chunk_commit!();
            fn_X_chunk_X_len_and_is_empty!(contiguous = $contiguous);

            fn_read_chunk_uninit_commit_unchecked!(contiguous = $contiguous);
        }
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
    (arc = yes) => {
        use alloc::boxed::Box;
        use core::ptr::NonNull;

        /// Non-public helper type.
        //#[derive(Debug, PartialEq, Eq)]
        struct ArcRingBuffer<T> {
            ptr: NonNull<RingBuffer<T>>,
        }

        // SAFETY: If RingBuffer is Send, ArcRingBuffer is as well.
        unsafe impl<T> Send for ArcRingBuffer<T> where RingBuffer<T>: Send {}

        impl<T> ArcRingBuffer<T> {
            #[allow(clippy::new_ret_no_self)]
            fn new(rb: RingBuffer<T>) -> (Producer<T>, Consumer<T>) {
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
        }

        impl<T> Drop for ArcRingBuffer<T> {
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
        }

        /// Non-inlined part of `Ref::drop()`.
        #[inline(never)]
        unsafe fn drop_slow<T>(ptr: NonNull<RingBuffer<T>>) {
            // SAFETY: This is allowed because the storage has been allocated with `Box::new()`.
            unsafe {
                // Turn the pointer back into a `Box` and immediately drop it,
                // which deallocates the memory allocated in `Ref::new()`.
                drop(Box::from_raw(ptr.as_ptr()));
            }
        }

        impl<T> core::ops::Deref for ArcRingBuffer<T> {
            type Target = RingBuffer<T>;

            fn deref(&self) -> &Self::Target {
                // SAFETY: There are never any mutable references.
                unsafe { self.ptr.as_ref() }
            }
        }
    };
    (arc = no) => {};
}

macro_rules! fn_ring_buffer_producer {
    (arc = yes, $producer:ty) => {};
    (arc = no, $producer:ty) => {
        pub fn producer(&self) -> Option<$producer> {
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
    (arc = yes, $consumer:ty) => {};
    (arc = no, $consumer:ty) => {
        pub fn consumer(&self) -> Option<$consumer> {
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

macro_rules! struct_producer {
    (arc = yes, N = ($($N:ident)?)) => {
        $(compile_error!(concat!("`N = (", stringify!($N), ")` is not supported with `arc = yes`"));)?

        // TODO: manual impls:
        //#[derive(Debug, PartialEq, Eq)]
        pub struct Producer<T> {
            buffer: ArcRingBuffer<T>,
            cached_head: Cell<usize>,
            cached_tail: Cell<usize>,
        }
    };
    (arc = no, N = ($($N:ident)?)) => {
        // TODO: manual impls:
        //#[derive(Debug, PartialEq, Eq)]
        pub struct Producer<'a, T$(, const $N: usize)?> {
            buffer: &'a RingBuffer<T$(, $N)?>,
            cached_head: Cell<usize>,
            cached_tail: Cell<usize>,
        }

        impl<T$(, const $N: usize)?> Drop for Producer<'_, T$(, $N)?>
        {
            fn drop(&mut self) {
                let _ = self.buffer.flags.fetch_and(!HAS_PRODUCER, Ordering::SeqCst);
            }
        }
    };
}

macro_rules! struct_consumer {
    (arc = yes, N = ($($N:ident)?)) => {
        // TODO: manual impls:
        //#[derive(Debug, PartialEq, Eq)]
        pub struct Consumer<T> {
            buffer: ArcRingBuffer<T>,
            cached_head: Cell<usize>,
            cached_tail: Cell<usize>,
        }
    };
    (arc = no, N = ($($N:ident)?)) => {
        // TODO: manual impls:
        //#[derive(Debug, PartialEq, Eq)]
        pub struct Consumer<'a, T$(, const $N: usize)?> {
            buffer: &'a RingBuffer<T$(, $N)?>,
            cached_head: Cell<usize>,
            cached_tail: Cell<usize>,
        }

        impl<T$(, const $N: usize)?> Drop for Consumer<'_, T$(, $N)?>
        {
            fn drop(&mut self) {
                let _ = self.buffer.flags.fetch_and(!HAS_CONSUMER, Ordering::SeqCst);
            }
        }
    };
}

macro_rules! fn_producer_push {
    () => {
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
        /// // TODO: module-specific example!
        /// use rtrb::{RingBuffer, PushError};
        ///
        /// let (mut p, c) = RingBuffer::new(1);
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
    () => {
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
        /// // TODO: module-specific example!
        /// use rtrb::RingBuffer;
        ///
        /// let (p, c) = RingBuffer::<f32>::new(1024);
        ///
        /// assert_eq!(p.slots(), 1024);
        /// ```
        pub fn slots(&self) -> usize {
            let b = &self.buffer;
            let head = b.head.load(Ordering::Acquire);
            self.cached_head.set(head);
            b.capacity() - b.distance(head, self.cached_tail.get())
        }
    };
}

macro_rules! fn_producer_is_full {
    () => {
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
        /// // TODO: module-specific example!
        /// use rtrb::RingBuffer;
        ///
        /// let (p, c) = RingBuffer::<f32>::new(1);
        ///
        /// assert!(!p.is_full());
        /// ```
        ///
        /// Since items can be concurrently consumed on another thread, the ring buffer
        /// might not be full for long:
        ///
        /// ```
        /// # use rtrb::RingBuffer;
        /// # let (p, c) = RingBuffer::<f32>::new(1);
        /// if p.is_full() {
        ///     // The buffer might be full, but it might as well not be
        ///     // if an item was just consumed on another thread.
        /// }
        /// ```
        ///
        /// However, if it's not full, another thread cannot change that:
        ///
        /// ```
        /// # use rtrb::RingBuffer;
        /// # let (p, c) = RingBuffer::<f32>::new(1);
        /// if !p.is_full() {
        ///     // At least one slot is guaranteed to be available for writing.
        /// }
        /// ```
        pub fn is_full(&self) -> bool {
            self.next_tail().is_none()
        }
    };
}

macro_rules! fn_pc_capacity {
    () => {
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
        /// // TODO: module-specific example!
        /// use rtrb::RingBuffer;
        ///
        /// let (producer, consumer) = RingBuffer::<f32>::new(100);
        /// assert_eq!(producer.capacity(), 100);
        /// assert_eq!(consumer.capacity(), 100);
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
    () => {
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
        /// // TODO: module-specific example!
        /// use rtrb::{PopError, RingBuffer};
        ///
        /// let (mut p, mut c) = RingBuffer::new(1);
        ///
        /// assert_eq!(p.push(10), Ok(()));
        /// assert_eq!(c.pop(), Ok(10));
        /// assert_eq!(c.pop(), Err(PopError::Empty));
        /// ```
        ///
        /// To obtain an [`Option<T>`](Option), use [`.ok()`](Result::ok) on the result.
        ///
        /// ```
        /// // TODO: module-specific example!
        /// # use rtrb::RingBuffer;
        /// # let (mut p, mut c) = RingBuffer::new(1);
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
    () => {
        /// Attempts to read an element from the queue without removing it.
        ///
        /// # Errors
        ///
        /// If the queue is empty, an error is returned.
        ///
        /// # Examples
        ///
        /// ```
        /// // TODO: module-specific example!
        /// use rtrb::{PeekError, RingBuffer};
        ///
        /// let (mut p, c) = RingBuffer::new(1);
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
    () => {
        "
Returns the number of slots available for reading.

Since items can be concurrently produced on another thread, the actual number
of available slots may increase at any time
(up to the [`capacity()`](Consumer::capacity)).

To check for a single available slot,
using [`is_empty()`](Consumer::is_empty) is often quicker
(because it might not have to check an atomic variable).

TODO: insert bip specifics

# Examples

```
// TODO: module-specific example!
use rtrb::RingBuffer;

let (p, c) = RingBuffer::<f32>::new(1024);

assert_eq!(c.slots(), 0);
```
"
    };
}

macro_rules! fn_consumer_slots {
    (bip = yes) => {
        #[doc = fn_consumer_slots_docstring!()]
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
    (bip = no) => {
        #[doc = fn_consumer_slots_docstring!()]
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
    () => {
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
        /// // TODO: module-specific example!
        /// use rtrb::RingBuffer;
        ///
        /// let (p, c) = RingBuffer::<f32>::new(1);
        ///
        /// assert!(c.is_empty());
        /// ```
        ///
        /// Since items can be concurrently produced on another thread, the ring buffer
        /// might not be empty for long:
        ///
        /// ```
        /// # use rtrb::RingBuffer;
        /// # let (p, c) = RingBuffer::<f32>::new(1);
        /// if c.is_empty() {
        ///     // The buffer might be empty, but it might as well not be
        ///     // if an item was just produced on another thread.
        /// }
        /// ```
        ///
        /// However, if it's not empty, another thread cannot change that:
        ///
        /// ```
        /// # use rtrb::RingBuffer;
        /// # let (p, c) = RingBuffer::<f32>::new(1);
        /// if !c.is_empty() {
        ///     // At least one slot is guaranteed to be available for reading.
        /// }
        /// ```
        pub fn is_empty(&self) -> bool {
            self.next_head().is_none()
        }
    };
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
    (bip = yes, $chunk:ty) => {
        pub fn write_chunk_uninit(&mut self, n: usize) -> Result<$chunk, ChunkError> {
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
    (bip = no, $chunk:ty) => {
        pub fn write_chunk_uninit(&mut self, n: usize) -> Result<$chunk, ChunkError> {
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
    (contiguous = $contiguous:ident, $chunk:ty) => {
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
        pub fn write_chunk(&mut self, n: usize) -> Result<$chunk, ChunkError>
        where
            T: Default,
        {
            self.write_chunk_uninit(n).map(WriteChunk::from)
        }
    };
}

macro_rules! fn_consumer_read_chunk {
    (bip = yes, $chunk:ty) => {
        pub fn read_chunk(&mut self, n: usize) -> Result<$chunk, ChunkError> {
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
    (bip = no, $chunk:ty) => {
        pub fn read_chunk(&mut self, n: usize) -> Result<$chunk, ChunkError> {
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
    () => {
        "

After writing to the slots, they are *not* automatically made available
to be read by the [`Consumer`].
This has to be explicitly done by calling [`commit()`](WriteChunkUninit::commit)
or [`commit_all()`](WriteChunkUninit::commit_all).
If items are written but *not* committed afterwards,
they will *not* become available for reading and
they will be leaked (which is only relevant if `T` implements [`Drop`]).
"
    };
}

macro_rules! fn_write_chunk_uninit_as_mut_sliceX {
    (contiguous = yes) => {
        /// Returns a slice for writing to the requested slots.
        ///
        /// The extension trait [`CopyToUninit`](crate::CopyToUninit) can be used
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
    () => {
        "

After writing to the slots, they are *not* automatically made available
to be read by the [`Consumer`].
This has to be explicitly done by calling [`commit()`](WriteChunk::commit)
or [`commit_all()`](WriteChunk::commit_all).
If items are written but *not* committed afterwards,
they will *not* become available for reading and
they will eventually be dropped (if `T` implements [`Drop`]).
"
    };
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
    () => {
        "

The provided slots are *not* automatically made available
to be written again by the [`Producer`].
This has to be explicitly done by calling [`commit()`](ReadChunk::commit)
or [`commit_all()`](ReadChunk::commit_all).
Note that this runs the destructor of the committed items (if `T` implements [`Drop`]).
You can \"peek\" at the contained values by simply not calling any of the \"commit\" methods.
"
    };
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
    () => {
        "

In the vast majority of cases, mutable access is not required when
reading data and the immutable version should be preferred. However,
there are some scenarios where it might be desirable to perform
operations on the data in-place without copying it to a separate buffer
(e.g. streaming decryption), in which case this version can be used.
"
    };
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
    (contiguous = yes , $producer:ty, N = ($($N:ident)?)) => {
        // TODO: implement manually:
        //#[derive(Debug, PartialEq, Eq)]
        pub struct WriteChunkUninit<'a, T$(, const $N: usize)?> {
            ptr: *mut T,
            len: usize,
            producer: &'a $producer,
        }

        impl<'a, T$(, const $N: usize)?> WriteChunkUninit<'a, T$(, $N)?> {
            unsafe fn new(producer: &'a $producer, n: usize, offset: usize) -> Self {
                Self {
                    // SAFETY: Caller must guarantee that `offset` is valid.
                    ptr: unsafe { producer.buffer.data_ptr().add(offset) },
                    len: n,
                    producer,
                }
            }
        }

        impl<'a, T$(, const $N: usize)?> From<WriteChunkUninit<'a, T$(, $N)?>> for WriteChunk<'a, T$(, $N)?>
        where
            T: Default,
        {
            /// Fills all slots with the [`Default`] value.
            fn from(chunk: WriteChunkUninit<'a, T$(, $N)?>) -> Self {
                for i in 0..chunk.len {
                    // SAFETY: i is in a valid range.
                    unsafe { chunk.ptr.add(i).write(Default::default()) };
                }
                WriteChunk(Some(chunk))
            }
        }
    };
    (contiguous = no , $producer:ty, N = ($($N:ident)?)) => {
        // TODO: implement manually:
        //#[derive(Debug, PartialEq, Eq)]
        pub struct WriteChunkUninit<'a, T$(, const $N: usize)?> {
            first_ptr: *mut T,
            first_len: usize,
            second_ptr: *mut T,
            second_len: usize,
            producer: &'a $producer,
        }

        impl<'a, T$(, const $N: usize)?> WriteChunkUninit<'a, T$(, $N)?> {
            unsafe fn new(producer: &'a $producer, n: usize, offset: usize) -> Self {
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
        }

        impl<'a, T$(, const $N: usize)?> From<WriteChunkUninit<'a, T$(, $N)?>> for WriteChunk<'a, T$(, $N)?>
        where
            T: Default,
        {
            /// Fills all slots with the [`Default`] value.
            fn from(chunk: WriteChunkUninit<'a, T$(, $N)?>) -> Self {
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
        }
    };
}

macro_rules! struct_write_chunk {
    (N = ($($N:ident)?)) => {
        //#[derive(Debug, PartialEq, Eq)]
        pub struct WriteChunk<'a, T$(, const $N: usize)?>(Option<WriteChunkUninit<'a, T$(, $N)?>>);

        impl<T$(, const $N: usize)?> Drop for WriteChunk<'_, T$(, $N)?> {
            fn drop(&mut self) {
                // NB: If `commit()` or `commit_all()` has been called, `self.0` is `None`.
                if let Some(mut chunk) = self.0.take() {
                    // No part of the chunk has been committed, all slots are dropped.
                    // SAFETY: All slots have been initialized in From::from().
                    unsafe { chunk.drop_suffix(0) };
                }
            }
        }
    };
}

macro_rules! struct_read_chunk {
    (contiguous = yes, $consumer:ty, N = ($($N:ident)?)) => {
        // TODO: implement manually:
        //#[derive(Debug, PartialEq, Eq)]
        pub struct ReadChunk<'a, T$(, const $N: usize)?> {
            ptr: *mut T,
            len: usize,
            consumer: &'a $consumer,
        }

        impl<'a, T$(, const $N: usize)?> ReadChunk<'a, T$(, $N)?> {
            unsafe fn new(consumer: &'a $consumer, n: usize, offset: usize) -> Self {
                Self {
                    // SAFETY: Caller must guarantee that `offset` is valid.
                    ptr: unsafe { consumer.buffer.data_ptr().add(offset) },
                    len: n,
                    consumer,
                }
            }
        }
    };
    (contiguous = no , $consumer:ty, N = ($($N:ident)?)) => {
        // TODO: implement manually:
        //#[derive(Debug, PartialEq, Eq)]
        pub struct ReadChunk<'a, T$(, const $N: usize)?> {
            // Must be "mut" for drop_in_place()
            first_ptr: *mut T,
            first_len: usize,
            // Must be "mut" for drop_in_place()
            second_ptr: *mut T,
            second_len: usize,
            consumer: &'a $consumer,
        }

        impl<'a, T$(, const $N: usize)?> ReadChunk<'a, T$(, $N)?> {
            unsafe fn new(consumer: &'a $consumer, n: usize, offset: usize) -> Self {
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
        }
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
    () => {
        "

Moves items from an iterator into the (uninitialized) slots of the chunk.

The number of moved items is returned.

All moved items are automatically made availabe to be read by the [`Consumer`].

# Examples

If the iterator contains too few items, only a part of the chunk
is made available for reading:

```
// TODO: select correct module
use rtrb::{RingBuffer, PopError};

let (mut p, mut c) = RingBuffer::new(4);

if let Ok(chunk) = p.write_chunk_uninit(3) {
    assert_eq!(chunk.fill_from_iter([10, 20]), 2);
} else {
    unreachable!();
}
assert_eq!(p.slots(), 2);
assert_eq!(c.pop(), Ok(10));
assert_eq!(c.pop(), Ok(20));
assert_eq!(c.pop(), Err(PopError::Empty));
```

If the chunk size is too small, some items may remain in the iterator.
To be able to keep using the iterator after the call,
`&mut` (or [`Iterator::by_ref()`]) can be used.

```
use rtrb::{RingBuffer, PopError};

let (mut p, mut c) = RingBuffer::new(4);

let mut it = vec![10, 20, 30].into_iter();
if let Ok(chunk) = p.write_chunk_uninit(2) {
    assert_eq!(chunk.fill_from_iter(&mut it), 2);
} else {
    unreachable!();
}
assert_eq!(c.pop(), Ok(10));
assert_eq!(c.pop(), Ok(20));
assert_eq!(c.pop(), Err(PopError::Empty));
assert_eq!(it.next(), Some(30));
```
"
    };
}

macro_rules! fn_write_chunk_uninit_fill_from_iter {
    (contiguous = yes) => {
        #[doc = fn_write_chunk_uninit_fill_from_iter_docstring!()]
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
    (contiguous = no) => {
        #[doc = fn_write_chunk_uninit_fill_from_iter_docstring!()]
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
    () => {
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
        /// // TODO: select correct module
        /// use rtrb::RingBuffer;
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
        ///     let (mut p, mut c) = RingBuffer::new(2);
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
    (N = ($($N:ident)?)) => {
        /// It (as well as [`WriteChunk`]) can be moved ...
        /// ```
        /// // TODO: select correct module
        /// fn assert_send<X: Send>() {}
        /// assert_send::<rtrb::chunks::WriteChunkUninit<u8>>();
        /// assert_send::<rtrb::chunks::WriteChunk<u8>>();
        /// ```
        /// ... but not shared between threads:
        /// ```compile_fail
        /// fn assert_sync<X: Sync>() {}
        /// assert_sync::<rtrb::chunks::WriteChunkUninit<u8>>();
        /// ```
        /// ```compile_fail
        /// # fn assert_sync<X: Sync>() {}
        /// assert_sync::<rtrb::chunks::WriteChunk<u8>>();
        /// ```
        // SAFETY: WriteChunkUninit only exists while a unique reference to the producer is held.
        // It is therefore safe to move it to another thread.
        unsafe impl<T: Send$(, const $N: usize)?> Send for WriteChunkUninit<'_, T$(, $N)?> {}

        /// It (and any wrapper structs) can be moved ...
        /// ```
        /// // TODO: select correct module
        /// fn assert_send<X: Send>() {}
        /// assert_send::<rtrb::chunks::ReadChunk<u8>>();
        /// ```
        /// ... but not shared between threads:
        /// ```compile_fail
        /// fn assert_sync<X: Sync>() {}
        /// assert_sync::<rtrb::chunks::ReadChunk<u8>>();
        /// ```
        // SAFETY: ReadChunk only exists while a unique reference to the consumer is held.
        // It is therefore safe to move it to another thread.
        unsafe impl<T: Send$(, const $N: usize)?> Send for ReadChunk<'_, T$(, $N)?> {}
    };
}
