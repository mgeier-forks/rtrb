// storage: vec, array, vrb, dst
// indices & calculation: mop, bip (bip+vrb doesn't make sense)
// indices & padding: tight, padded
// chunks: vrb is a special case: only contiguous; bip could have both?
// owning p&c: vec, vrb, dst; non-owning: array, maybe dst? "owned" module?
// addressing: double size, pow2, single size, pow2-single; one_less (waste_one), unwrap; pow2; not_pow2
// size_type: usize, u32, u16, u8; (u64 and u128 probably don't make sense?)

macro_rules! storage_vec {
    (padded = $padded:ident, bip = $bip:ident, rb_doc = $rb_doc:expr) => {
        use crate::atomic::*;
        use crate::CachePadded;
        use alloc::vec::Vec;
        use core::mem::ManuallyDrop;

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
            // Wraps from any number to the range `0 .. capacity`.
            fn collapse_position(&self, pos: usize) -> usize {
                // TODO: is capacity 0 supported?
                pos & (self.capacity() - 1)
            }

            /// Increments a position by going `n` slots forward.
            fn increment(&self, pos: usize, n: usize) -> usize {
                pos.wrapping_add(n)
            }

            /// Increments a position by going one slot forward.
            ///
            /// This might be more efficient than self.increment(..., 1).
            fn increment1(&self, pos: usize) -> usize {
                pos.wrapping_add(1)
            }

            /// Returns the distance between two positions.
            fn distance(&self, a: usize, b: usize) -> usize {
                b.wrapping_sub(a)
            }
        }
    };
}

// TODO: bip option for cached_skip?
macro_rules! def_producer_consumer_boxed {
    () => {
        use core::cell::Cell;

        // TODO: manual impls:
        //#[derive(Debug, PartialEq, Eq)]
        pub struct Producer<T> {
            buffer: BoxedRingBuffer<T>,
            cached_head: Cell<usize>,
            cached_tail: Cell<usize>,
            // TODO: cached_skip?
            // NB: caching `skip` doesn't help, because it can jump to any position.
        }

        // TODO: manual impls:
        //#[derive(Debug, PartialEq, Eq)]
        pub struct Consumer<T> {
            buffer: BoxedRingBuffer<T>,
            cached_head: Cell<usize>,
            cached_tail: Cell<usize>,
            // TODO: cached_skip?
        }
    };
}

macro_rules! def_boxed_ring_buffer {
    () => {
        use alloc::boxed::Box;
        use core::ptr::NonNull;

        /// Non-public helper type.
        //#[derive(Debug, PartialEq, Eq)]
        struct BoxedRingBuffer<T> {
            ptr: NonNull<RingBuffer<T>>,
        }

        unsafe impl<T: Send> Send for BoxedRingBuffer<T> {}

        impl<T> BoxedRingBuffer<T> {
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

        impl<T> Drop for BoxedRingBuffer<T> {
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

        impl<T> core::ops::Deref for BoxedRingBuffer<T> {
            type Target = RingBuffer<T>;

            fn deref(&self) -> &Self::Target {
                // SAFETY: There are never any mutable references.
                unsafe { self.ptr.as_ref() }
            }
        }
    };
}

// TODO: bip option for cached_skip?
macro_rules! def_producer_consumer_ref {
    (N = ($($N:ident)?)) => {
        use core::cell::Cell;

        // TODO: manual impls:
        //#[derive(Debug, PartialEq, Eq)]
        pub struct Producer<'a, T$(, const $N: usize)?> {
            buffer: &'a RingBuffer<T$(, $N)?>,
            cached_head: Cell<usize>,
            cached_tail: Cell<usize>,
            // TODO: cached_skip?
        }

        // TODO: manual impls:
        //#[derive(Debug, PartialEq, Eq)]
        pub struct Consumer<'a, T$(, const $N: usize)?> {
            buffer: &'a RingBuffer<T$(, $N)?>,
            cached_head: Cell<usize>,
            cached_tail: Cell<usize>,
            // TODO: cached_skip?
        }

        use crate::diy::{HAS_CONSUMER, HAS_PRODUCER};

        impl<T$(, const $N: usize)?> RingBuffer<T$(, $N)?> {
            pub fn producer(&self) -> Option<Producer<T$(, $N)?>> {
                let old_flags = self.flags.fetch_or(HAS_PRODUCER, Ordering::SeqCst);
                if old_flags & HAS_PRODUCER == 0 {
                    let head = self.head.load(Ordering::Acquire);
                    let tail = self.tail.load(Ordering::Acquire);
                    Some(
                        Producer {
                            buffer: self,
                            cached_head: Cell::new(head),
                            cached_tail: Cell::new(tail),
                        }
                    )
                } else {
                    None
                }
            }

            pub fn consumer(&self) -> Option<Consumer<T$(, $N)?>> {
                let old_flags = self.flags.fetch_or(HAS_CONSUMER, Ordering::SeqCst);
                if old_flags & HAS_CONSUMER == 0 {
                    let head = self.head.load(Ordering::Acquire);
                    let tail = self.tail.load(Ordering::Acquire);
                    Some(
                        Consumer{
                            buffer: self,
                            cached_head: Cell::new(head),
                            cached_tail: Cell::new(tail),
                        }
                    )
                } else {
                    None
                }
            }
        }

        impl<T$(, const $N: usize)?> Drop for Producer<'_, T$(, $N)?>
        {
            fn drop(&mut self) {
                let _ = self.buffer.flags.fetch_and(!HAS_PRODUCER, Ordering::SeqCst);
            }
        }

        impl<T$(, const $N: usize)?> Drop for Consumer<'_, T$(, $N)?>
        {
            fn drop(&mut self) {
                let _ = self.buffer.flags.fetch_and(!HAS_CONSUMER, Ordering::SeqCst);
            }
        }
    };
}

/// NB: next_tail() can also be used for "bip", because `b.skip` is never set.
/// One element can always be inserted without skipping.
// TODO: move this into impl_common?
macro_rules! impl_producer_consumer_common {
    ('a = ($($a:lifetime)?), N = ($($N:ident)?)) => {
        impl<$($a, )?T$(, const $N: usize)?> Producer<$($a, )?T$(, $N)?> {
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
            // NB: This also works for "bip", since `skip` is irrelevant for `push()`.
            pub fn slots(&self) -> usize {
                let b = &self.buffer;
                let head = b.head.load(Ordering::Acquire);
                self.cached_head.set(head);
                b.capacity() - b.distance(head, self.cached_tail.get())
            }

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
        }

        impl<$($a, )?T$(, const $N: usize)?> Consumer<$($a, )?T$(, $N)?> {
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
        }
    }
}

// TODO: combine with other macros?
macro_rules! impl_producer_consumer_bip {
    ('a = ($($a:lifetime)?), N = ($($N:ident)?)) => {

        // Disable skipping (0 is an impossible value for `skip`).
        // TODO: move to a more meaningful place?
        pub const NO_SKIP: usize = 0;

        impl<$($a, )?T$(, const $N: usize)?> Consumer<$($a, )?T$(, $N)?> {
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
            /// TODO: [`read_chunk()`](Consumer::read_chunk) might not provide the full number of free slots
            ///
            /// TODO: see alternative "slots" variations
            ///
            /// # Examples
            ///
            /// ```
            /// // TODO: module-specific example!
            /// use rtrb::RingBuffer;
            ///
            /// let (p, c) = RingBuffer::<f32>::new(1024);
            ///
            /// assert_eq!(c.slots(), 0);
            /// ```
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
                    let end = if skip != NO_SKIP {
                        b.collapse_position(skip)
                    } else {
                        b.capacity()
                    };
                    collapsed_tail + end - collapsed_head
                }
            }
        }
    }
}

macro_rules! impl_next_head_non_bip {
    ('a = ($($a:lifetime)?), N = ($($N:ident)?)) => {
        impl<$($a, )?T$(, const $N: usize)?> Consumer<$($a, )?T$(, $N)?> {
            /// Get the head position for reading the next slot, if available.
            ///
            /// This is a strict subset of the functionality implemented in `read_chunk()`.
            /// For performance, this special case is implemented separately.
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
        }
    }
}

// TODO: move this somewhere
macro_rules! impl_next_head_bip {
    ('a = ($($a:lifetime)?), N = ($($N:ident)?)) => {
        impl<$($a, )?T$(, const $N: usize)?> Consumer<$($a, )?T$(, $N)?> {
            /// Get the `head` position for reading the next slot, if available.
            ///
            /// This is a strict subset of the functionality implemented in `read_chunk()`.
            /// For performance, this special case is implemented separately.
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

                // TODO: check if caching skip is worth it.
                /*
                // TODO: skip = cached_skip;
                if skip != NO_SKIP && head == skip {
                    // we maybe have to skip, but maybe not
                } else {
                    // we maybe don't have to skip, but how can loading skip change that?
                }
                */

                if b.collapse_position(tail) < b.collapse_position(head) {
                    // NB: We are only allowed to use `skip` if (collapsed) `tail < head`.
                    let mut skip = b.skip.load(Ordering::Acquire);
                    if skip != NO_SKIP && head == skip {
                        // Nothing to read at the end of the buffer, wrap `head` and clear `skip`.
                        head = b.increment(head, b.capacity() - b.collapse_position(skip));
                        b.head.store(head, Ordering::Release);
                        self.cached_head.set(head);
                        skip = NO_SKIP;
                        b.skip.store(skip, Ordering::Release);

                        // NB: The producer only sets `skip` if it writes at least one slot
                        // at the beginning of the buffer.  Therefore, we know that the
                        // wrapped-around `head` is valid for reading (at least) one slot.
                    }
                }
                Some(head)
            }
        }
    };
}

macro_rules! impl_chunks_mop {
    ('a = ($($a:lifetime)?), N = ($($N:ident)?)) => {
        impl<T$(, const $N: usize)?> ReadChunk<'_, T$(, $N)?> {
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
        }
    }
}

// mop and vrb
macro_rules! impl_chunks_non_bip {
    ('a = ($($a:lifetime)?), N = ($($N:ident)?)) => {
        impl<T$(, const $N: usize)?> WriteChunkUninit<'_, T$(, $N)?> {
            unsafe fn commit_unchecked(self, n: usize) -> usize {
                let p = self.producer;
                let tail = p.buffer.increment(p.cached_tail.get(), n);
                p.buffer.tail.store(tail, Ordering::Release);
                p.cached_tail.set(tail);
                n
            }
        }
    }
}

macro_rules! impl_chunks_bip {
    ('a = ($($a:lifetime)?), N = ($($N:ident)?)) => {
        impl<T$(, const $N: usize)?> WriteChunkUninit<'_, T$(, $N)?> {
            unsafe fn commit_unchecked(self, n: usize) -> usize {
                if n == 0 {
                    // NB: No slots will be skipped, both `tail` and `skip` remain unchanged.
                    // This is the same as if the function wasn't called at all.
                    return n;
                }
                let b = &self.producer.buffer;
                let mut tail = self.producer.cached_tail.get();
                if self.ptr == b.data_ptr() && b.collapse_position(tail) != 0 {
                    // NB: It is safe to store `skip` before `tail`, because the consumer
                    // will potentially only read between `head` and (the old) `tail`,
                    // without looking at `skip`.
                    // Storing `tail` before `skip` would be problematic, however, because
                    // the consumer would see new data at the beginning of the buffer,
                    // but wouldn't know that the end has to be skipped.
                    b.skip.store(tail, Ordering::Release);
                    // TODO: make this a reusable function?
                    tail = b.increment(tail, b.capacity() - b.collapse_position(tail));
                }
                tail = b.increment(tail, n);
                b.tail.store(tail, Ordering::Release);
                self.producer.cached_tail.set(tail);
                n
            }
        }

        // TODO: separate version for non-bip but contiguous (i.e. vrb)
        impl<T$(, const $N: usize)?> ReadChunk<'_, T$(, $N)?> {
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
        }

        impl<$($a, )?T$(, const $N: usize)?> Producer<$($a, )?T$(, $N)?> {
            pub fn write_chunk(&mut self, n: usize) -> Result<WriteChunk<'_, T$(, $N)?>, ChunkError>
            where
                T: Default,
            {
                self.write_chunk_uninit(n).map(WriteChunk::from)
            }

            pub fn write_chunk_uninit(
                &mut self,
                n: usize,
            ) -> Result<WriteChunkUninit<'_, T$(, $N)?>, ChunkError> {
                let mut head = self.cached_head.get();
                let tail = self.cached_tail.get();
                let b = &self.buffer;
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
                Ok(WriteChunkUninit {
                    // SAFETY: `offset` has been set to a valid position.
                    ptr: unsafe { b.data_ptr().add(offset) },
                    len: n,
                    producer: self,
                })
            }
        }

        impl<$($a, )?T$(, const $N: usize)?> Consumer<$($a, )?T$(, $N)?> {
            pub fn read_chunk(&mut self, n: usize) -> Result<ReadChunk<'_, T$(, $N)?>, ChunkError> {
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
                    let mut skip = b.skip.load(Ordering::Acquire);
                    let end = if skip == NO_SKIP {
                        b.capacity()
                    } else {
                        b.collapse_position(skip)
                    };
                    slots = end - collapsed_head;
                    if slots == 0 {
                        // No more slots at the end of the buffer, let's wrap around.
                        if skip != NO_SKIP {
                            skip = NO_SKIP;
                            b.skip.store(skip, Ordering::Release);
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
                Ok(ReadChunk {
                    // SAFETY: ...
                    ptr: unsafe { b.data_ptr().add(offset) },
                    len: n,
                    consumer: self,
                })
            }
        }
    };
}

macro_rules! impl_chunks_non_contiguous {
    ('a = ($($a:lifetime)?), N = ($($N:ident)?)) => {
        use core::mem::MaybeUninit;

        //#[derive(Debug, PartialEq, Eq)]
        pub struct WriteChunkUninit<'a, T$(, const $N: usize)?> {
            first_ptr: *mut T,
            first_len: usize,
            second_ptr: *mut T,
            second_len: usize,
            producer: &'a Producer<$($a, )?T$(, $N)?>,
        }

        impl<T$(, const $N: usize)?> WriteChunkUninit<'_, T$(, $N)?> {
            pub fn as_mut_slices(&mut self) -> (&mut [MaybeUninit<T>], &mut [MaybeUninit<T>]) {
                // SAFETY: The pointers and lengths have been computed correctly in write_chunk_uninit().
                unsafe {
                    (
                        core::slice::from_raw_parts_mut(self.first_ptr.cast(), self.first_len),
                        core::slice::from_raw_parts_mut(self.second_ptr.cast(), self.second_len),
                    )
                }
            }

            /// Drops all elements starting from index `n`.
            ///
            /// #Safety
            ///
            /// All of those slots must be initialized.
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

            pub fn len(&self) -> usize {
                self.first_len + self.second_len
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

        impl<T$(, const $N: usize)?> WriteChunk<'_, T$(, $N)?>
        where
            T: Default,
        {
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
        }

        //#[derive(Debug, PartialEq, Eq)]
        pub struct ReadChunk<'a, T$(, const $N: usize)?> {
            // Must be "mut" for drop_in_place()
            first_ptr: *mut T,
            first_len: usize,
            // Must be "mut" for drop_in_place()
            second_ptr: *mut T,
            second_len: usize,
            consumer: &'a Consumer<$($a, )?T$(, $N)?>,
        }

        impl<T$(, const $N: usize)?> ReadChunk<'_, T$(, $N)?> {
            pub fn as_slices(&self) -> (&[T], &[T]) {
                // SAFETY: The pointers and lengths have been computed correctly in read_chunk().
                unsafe {
                    (
                        core::slice::from_raw_parts(self.first_ptr, self.first_len),
                        core::slice::from_raw_parts(self.second_ptr, self.second_len),
                    )
                }
            }

            pub fn as_mut_slices(&mut self) -> (&mut [T], &mut [T]) {
                // SAFETY: The pointers and lengths have been computed correctly in read_chunk().
                unsafe {
                    (
                        core::slice::from_raw_parts_mut(self.first_ptr, self.first_len),
                        core::slice::from_raw_parts_mut(self.second_ptr, self.second_len),
                    )
                }
            }

            pub fn len(&self) -> usize {
                self.first_len + self.second_len
            }

            pub fn is_empty(&self) -> bool {
                self.first_len == 0
            }
        }
    }
}

// bip and vrb
macro_rules! impl_chunks_contiguous {
    ('a = ($($a:lifetime)?), N = ($($N:ident)?)) => {
        use core::mem::MaybeUninit;

        //#[derive(Debug, PartialEq, Eq)]
        pub struct WriteChunkUninit<'a, T$(, const $N: usize)?> {
            ptr: *mut T,
            len: usize,
            producer: &'a Producer<$($a, )?T$(, $N)?>,
        }

        impl<T$(, const $N: usize)?> WriteChunkUninit<'_, T$(, $N)?> {
            pub fn as_mut_slice(&mut self) -> &mut [MaybeUninit<T>] {
                // SAFETY: The pointer and length have been computed correctly in write_chunk_uninit().
                unsafe { core::slice::from_raw_parts_mut(self.ptr.cast(), self.len) }
            }

            /// Drops all elements starting from index `n`.
            ///
            /// #Safety
            ///
            /// All of those slots must be initialized.
            unsafe fn drop_suffix(&mut self, n: usize) {
                // NB: If n >= self.len(), the loop is not entered.
                for i in n..self.len {
                    // SAFETY: The caller must make sure that all slots are initialized.
                    unsafe { self.ptr.add(i).drop_in_place() };
                }
            }

            pub fn len(&self) -> usize {
                self.len
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

        impl<T$(, const $N: usize)?> WriteChunk<'_, T$(, $N)?>
        where
            T: Default,
        {
            pub fn as_mut_slice(&mut self) -> &mut [T] {
                // self.0 is always Some(chunk).
                let chunk = self.0.as_ref().unwrap();
                // SAFETY: The pointer and length have been computed correctly in write_chunk_uninit()
                // and all slots have been initialized in From::from().
                unsafe { core::slice::from_raw_parts_mut(chunk.ptr, chunk.len) }
            }
        }

        //#[derive(Debug, PartialEq, Eq)]
        pub struct ReadChunk<'a, T$(, const $N: usize)?> {
            ptr: *mut T,
            len: usize,
            consumer: &'a Consumer<$($a, )?T$(, $N)?>,
        }

        impl<T$(, const $N: usize)?> ReadChunk<'_, T$(, $N)?> {
            pub fn as_slice(&self) -> &[T] {
                // SAFETY: The correct pointer and length have been provided by ReadChunk::new().
                unsafe { core::slice::from_raw_parts(self.ptr, self.len) }
            }

            pub fn as_mut_slice(&mut self) -> &mut [T] {
                // SAFETY: The correct pointer and length have been provided by ReadChunk::new().
                unsafe { core::slice::from_raw_parts_mut(self.ptr, self.len) }
            }

            pub fn len(&self) -> usize {
                self.len
            }

            pub fn is_empty(&self) -> bool {
                self.len == 0
            }
        }
    }
}

macro_rules! impl_chunks_common {
    (N = ($($N:ident)?)) => {
        /// It (as well as [`WriteChunk`]) can be moved ...
        /// ```
        /// // TODO: select correct module
        /// fn assert_send<X: Send>() {}
        /// assert_send::<rtrb::chunks::WriteChunkUninit<u8>>();
        /// ```
        /// ... but not shared between threads:
        /// ```compile_fail
        /// fn assert_sync<X: Sync>() {}
        /// assert_sync::<rtrb::chunks::WriteChunkUninit<u8>>();
        /// ```
        // SAFETY: WriteChunkUninit only exists while a unique reference to the producer is held.
        // It is therefore safe to move it to another thread.
        unsafe impl<T: Send$(, const $N: usize)?> Send for WriteChunkUninit<'_, T$(, $N)?> {}

        impl<T$(, const $N: usize)?> WriteChunkUninit<'_, T$(, $N)?> {
            pub unsafe fn commit_all(self) {
                let slots = self.len();
                // SAFETY: Delegated to the caller.
                unsafe { self.commit_unchecked(slots) };
            }
        }

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

        impl<T$(, const $N: usize)?> WriteChunk<'_, T$(, $N)?>
        where
            T: Default,
        {
            pub fn commit_all(mut self) {
                // self.0 is always Some(chunk).
                let chunk = self.0.take().unwrap();
                // SAFETY: All slots have been initialized in From::from().
                unsafe { chunk.commit_all() };
                // `self` is dropped here, with `self.0` being set to `None`.
            }
        }


        impl<T$(, const $N: usize)?> ReadChunk<'_, T$(, $N)?> {
            pub fn commit_all(self) {
                let slots = self.len();
                // SAFETY: self.len() initialized elements have been obtained in read_chunk().
                unsafe { self.commit_unchecked(slots) };
            }
        }
    };
}
