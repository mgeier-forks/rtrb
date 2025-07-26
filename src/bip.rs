//! A bi-partite ring buffer.
//!
//! Simon Cooke (2003)
//! <https://www.codeproject.com/Articles/3479/The-Bip-Buffer-The-Circular-Buffer-with-a-Twist>
//! (not thread-safe)
//!
//! two revolving regions
//!
//! "two-phase allocation system" (reserve + commit)
//!
//! history:
//! "The FIFO logic can tell if the FIFO is empty because the head and tail values are the same, and it's full if the head is one greater than the tail."
//!
//! "Once more free space is available to the left of region A than to the right of it, a second region (comically named "region B") is created in that space."
//!
//! Reserve -> Commit; GetContiguousBlock -> DecommitBlock.
//!
//! 2019:
//! <https://ferrous-systems.com/blog/lock-free-ring-buffer/>
//! <https://blog.systems.ethz.ch/blog/2019/the-design-and-implementation-of-a-lock-free-ring-buffer-with-contiguous-reservations.html>
//!
//! Other Rust implementations:
//! <https://crates.io/crates/bbqueue>
//! <https://crates.io/crates/bipbuffer> (not thread-safe)
//! <https://crates.io/crates/spsc-bip-buffer>
//!
//! Implementations in other languages:
//! <https://github.com/willemt/bipbuffer> (C)

use alloc::{boxed::Box, vec::Vec};
use core::{
    cell::Cell,
    mem::ManuallyDrop,
    ptr::NonNull,
    sync::atomic::{AtomicU8, AtomicUsize, Ordering},
};

use crate::{
    cache_padded::CachePadded, chunks::ChunkError, diy::IS_ABANDONED, PeekError, PopError,
    PushError,
};

// Disable skipping (0 is an impossible value for `skip`).
const NO_SKIP: usize = 0;

storage_vec! {
    padded = yes,
    bip = yes,
    rb_doc = "
Bi-partite ring buffer.

TODO: some more docs, maybe links? [`RingBuffer::new()`].

*See also the [module-level documentation](crate::bip).*
"
}

impl_drop_all_elements! {
    bip = yes,
    N = ()
}

impl_common! {
    N = ()
}

impl_calculation! {
    pow2 = no,
    N = ()
}

/*
impl<T, const C: u8, I: Indices> PartialEq for BipStorage<T, C, I> {
    fn eq(&self, other: &Self) -> bool {
        core::ptr::eq(self, other)
    }
}

impl<T, const C: u8, I: Indices> Eq for BipStorage<T, C, I> {}
*/

// TODO: manual impls:
//#[derive(Debug, PartialEq, Eq)]
pub struct Producer<T> {
    buffer: BoxedRingBuffer<T>,
    cached_head: Cell<usize>,
    cached_tail: Cell<usize>,
    // NB: caching `skip` doesn't help, because it can jump to any position.
}

impl<T> Producer<T> {
    // TODO: same as rtrb::Producer?
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

    /// Get the tail position for writing the next slot, if available.
    ///
    /// This is a strict subset of the functionality implemented in `write_chunk_uninit()`.
    /// For performance, this special case is implemented separately.
    // TODO: same as rtrb::Producer? (except for comment)
    fn next_tail(&self) -> Option<usize> {
        let head = self.cached_head.get();
        let tail = self.cached_tail.get();
        let b = &self.buffer;

        // NB: `b.skip` is never set. One element can always be inserted without skipping.

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
    pub fn slots(&self) -> usize {
        todo!()
    }
    /// The maximum number of slots for contiguous writing.
    // TODO: return a pair? or the max?
    pub fn slots_contiguous(&self) -> usize {
        todo!()
    }
    // TODO: different kinds of slots() functions? first and second, only first?
    pub fn slots_contiguous1(&self) -> usize {
        todo!()
    }
    // TODO: this is probably not meaningful? only "first" and "max"?
    pub fn slots_contiguous2(&self) -> usize {
        todo!()
    }
    pub fn slots_one(&self) -> usize {
        todo!()
    }
    pub fn slots_two(&self) -> usize {
        todo!()
    }
    // TODO: disable public is_full for bip?
    // not useful for contiguous chunks!?!
    pub fn is_full(&self) -> bool {
        todo!()
    }
    // TODO: disable public capacity for bip?
    pub fn capacity(&self) -> usize {
        todo!()
    }
    pub fn is_abandoned(&self) -> bool {
        todo!()
    }
}

// TODO: manual impls:
//#[derive(Debug, PartialEq, Eq)]
pub struct Consumer<T> {
    buffer: BoxedRingBuffer<T>,
    cached_head: Cell<usize>,
    cached_tail: Cell<usize>,
    // TODO: cached_skip?
}

impl<T> Consumer<T> {
    // TODO: same as rtrb::Consumer?
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

    /// Get the `head` position for reading the next slot, if available.
    ///
    /// This is a strict subset of the functionality implemented in `read_chunk()`.
    /// For performance, this special case is implemented separately.
    fn next_head(&self) -> Option<usize> {
        let mut head = self.cached_head.get();
        let mut tail = self.cached_tail.get();
        let b = &self.buffer;

        let mut tail_has_been_refreshed = false;
        // Check if the queue is *possibly* empty.
        if head == tail {
            // Refresh the tail ...
            tail = b.tail.load(Ordering::Acquire);
            self.cached_tail.set(tail);
            tail_has_been_refreshed = true;
            // ... and check if it's *really* empty.
            if head == tail {
                // `tail` didn't change, queue is empty.
                return None;
            } else if b.collapse_position(head) < b.collapse_position(tail) {
                // `tail` did change, but it didn't wrap around.
                return Some(head);
            }
        }
        // `tail` potentially wrapped around, so we have to check `skip`.
        let mut skip = b.skip.load(Ordering::Acquire);
        if skip != NO_SKIP && head == skip {
            head = b.increment(head, b.capacity() - b.collapse_position(skip));
            b.head.store(head, Ordering::Release);
            self.cached_head.set(head);
            skip = NO_SKIP;
            b.skip.store(skip, Ordering::Release);
            if head == tail {
                if tail_has_been_refreshed {
                    return None;
                }
                tail = b.tail.load(Ordering::Acquire);
                self.cached_tail.set(tail);
                if head == tail {
                    return None;
                }
            }
        }
        Some(head)
    }

    pub fn peek(&self) -> Result<&T, PeekError> {
        todo!()
    }
    pub fn slots(&self) -> usize {
        todo!()
    }
    // TODO: disable public is_empty for bip?
    pub fn is_empty(&self) -> bool {
        todo!()
    }
    pub fn is_abandoned(&self) -> bool {
        todo!()
    }
    // TODO: disable public capacity for bip?
    pub fn capacity(&self) -> usize {
        todo!()
    }
}

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

// "chunks" stuff. make separate module or not?

//#[derive(Debug, PartialEq, Eq)]
pub struct ContiguousWriteChunkUninit<'a, T> {
    ptr: *mut T,
    len: usize,
    producer: &'a Producer<T>,
}

impl<T> ContiguousWriteChunkUninit<'_, T> {
    unsafe fn commit_unchecked(self, n: usize) -> usize {
        if n == 0 {
            // NB: No slots will be skipped, both `tail` and `skip` remain unchanged.
            return n;
        }
        let b = &self.producer.buffer;
        let mut tail = self.producer.cached_tail.get();
        if self.ptr == b.data_ptr() && b.collapse_position(tail) != 0 {
            b.skip.store(tail, Ordering::Release);
            // TODO: make this a reusable function?
            tail = b.increment(tail, b.capacity() - b.collapse_position(tail));
            // NB: It is safe to store `skip` before `tail`, because the consumer
            // will potentially only read between `head` and (the old) `tail`,
            // without looking at `skip`.
            // Storing `tail` before `skip` would be problematic, however, because
            // the consumer would see new data at the beginning of the buffer,
            // but wouldn't know that the end has to be skipped.
        }
        tail = b.increment(tail, n);
        b.tail.store(tail, Ordering::Release);
        self.producer.cached_tail.set(tail);
        n
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

// TODO: same as non-contiguous
impl<T> ContiguousWriteChunkUninit<'_, T> {
    pub unsafe fn commit_all(self) {
        let slots = self.len();
        // SAFETY: Delegated to the caller.
        unsafe { self.commit_unchecked(slots) };
    }
}

// TODO: same as non-contiguous?
//#[derive(Debug, PartialEq, Eq)]
pub struct ContiguousWriteChunk<'a, T>(Option<ContiguousWriteChunkUninit<'a, T>>);

// TODO: same as non-contiguous?
impl<T> Drop for ContiguousWriteChunk<'_, T> {
    fn drop(&mut self) {
        // NB: If `commit()` or `commit_all()` has been called, `self.0` is `None`.
        if let Some(mut chunk) = self.0.take() {
            // No part of the chunk has been committed, all slots are dropped.
            // SAFETY: All slots have been initialized in From::from().
            unsafe { chunk.drop_suffix(0) };
        }
    }
}

impl<'a, T> From<ContiguousWriteChunkUninit<'a, T>> for ContiguousWriteChunk<'a, T>
where
    T: Default,
{
    /// Fills all slots with the [`Default`] value.
    fn from(chunk: ContiguousWriteChunkUninit<'a, T>) -> Self {
        for i in 0..chunk.len {
            // SAFETY: i is in a valid range.
            unsafe { chunk.ptr.add(i).write(Default::default()) };
        }
        ContiguousWriteChunk(Some(chunk))
    }
}

// TODO: same as non-contiguous
impl<T> ContiguousWriteChunk<'_, T>
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

//#[derive(Debug, PartialEq, Eq)]
pub struct ContiguousReadChunk<'a, T> {
    ptr: *mut T,
    len: usize,
    consumer: &'a Consumer<T>,
}

impl<T> ContiguousReadChunk<'_, T> {
    pub fn as_slice(&self) -> &[T] {
        todo!()
    }

    pub fn as_mut_slice(&mut self) -> &mut [T] {
        todo!()
    }

    pub fn commit(self, n: usize) {
        todo!()
    }

    pub fn commit_all(self) {
        todo!()
    }

    pub fn len(&self) -> usize {
        todo!()
    }

    pub fn is_empty(&self) -> bool {
        todo!()
    }

    unsafe fn commit_unchecked(self, n: usize) -> usize {
        // TODO: if skip is reached -> reset skip? set head to beginning

        let len = self.len.min(n);
        for i in 0..len {
            // SAFETY: The caller must make sure that there are n initialized elements.
            unsafe { self.ptr.add(i).drop_in_place() };
        }
        let c = self.consumer;
        let head = c.buffer.increment(c.cached_head.get(), n);
        c.buffer.head.store(head, Ordering::Release);
        c.cached_head.set(head);
        n
    }
}

impl<T> Producer<T> {
    pub fn write_chunk(&mut self, n: usize) -> Result<ContiguousWriteChunk<'_, T>, ChunkError>
    where
        T: Default,
    {
        self.write_chunk_uninit(n).map(ContiguousWriteChunk::from)
    }
    pub fn write_chunk_uninit(
        &mut self,
        n: usize,
    ) -> Result<ContiguousWriteChunkUninit<'_, T>, ChunkError> {
        let mut head = self.cached_head.get();
        let tail = self.cached_tail.get();
        let b = &self.buffer;
        // TODO: check if everything is compatible with power-of-2 addressing.
        let mut slots = 0;
        let mut head_has_been_refreshed = false;
        // Collapsing the indices makes it impossible to distinguish empty and full,
        // so we check for emptiness first.
        let is_empty = head == tail;
        if !is_empty && b.collapse_position(tail) <= b.collapse_position(head) {
            // Is there enough space between `tail` and `head`?
            slots = head - tail;
            if slots < n {
                // Refresh head ...
                head = b.head.load(Ordering::Acquire);
                self.cached_head.set(head);
                head_has_been_refreshed = true;
                // ... and try again.
                let is_empty = head == tail;
                if !is_empty && b.collapse_position(tail) <= b.collapse_position(head) {
                    // `head` did not wrap around.
                    slots = head - tail;
                    if slots < n {
                        return Err(ChunkError::TooFewSlots(slots));
                    }
                } else {
                    // `head` did wrap around, we'll continue below.
                }
            }
        }
        let offset;
        if slots < n {
            // Is there enough space at the end of the buffer?
            slots = b.capacity() - b.collapse_position(tail);
            if slots < n {
                // Nope, let's check the beginning.

                // TODO: interaction/reuse with slots() et al.?

                slots = slots.max(b.collapse_position(head));
                if slots < n {
                    // TODO: check if this early return/local variable is an actual optimization?
                    if head_has_been_refreshed {
                        return Err(ChunkError::TooFewSlots(slots));
                    }
                    head = b.head.load(Ordering::Acquire);
                    self.cached_head.set(head);
                    slots = slots.max(b.collapse_position(head));
                    if slots < n {
                        return Err(ChunkError::TooFewSlots(slots));
                    }
                }
                // NB: `tail` will be (conditionally) reset in `commit_unchecked()`.
                offset = 0;
            } else {
                offset = b.collapse_position(tail);
            }
        } else {
            offset = b.collapse_position(tail);
        }
        Ok(ContiguousWriteChunkUninit {
            // SAFETY: tail has been updated to a valid position.
            ptr: unsafe { b.data_ptr().add(offset) },
            len: n,
            producer: self,
        })
    }
}

impl<T> Consumer<T> {
    pub fn read_chunk(&mut self, n: usize) -> Result<ContiguousReadChunk<'_, T>, ChunkError> {
        let b = &self.buffer;
        let head = self.cached_head.get();
        let mut tail = self.cached_tail.get();
        let mut slots = 0;
        // TODO: what happens when queue is empty/full at this point?
        if b.collapse_position(head) <= b.collapse_position(tail) {
            slots = tail - head;
            if slots < n {
                // Refresh the tail ...
                tail = b.tail.load(Ordering::Acquire);
                self.cached_tail.set(tail);
                // ... and check again.
                if b.collapse_position(head) < b.collapse_position(tail) {
                    // `tail` did not wrap around.
                    slots = tail - head;
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
            let mut skip = b.skip.load(Ordering::Acquire);
            if skip == NO_SKIP {
                // TODO: does this work at wrap-around with power-of-2 addressing?
                skip = b.increment(head, b.capacity() - b.collapse_position(head));
            }
            // TODO: maybe use wrapping_sub()? use distance()? see also subtractions above!
            slots = skip - head;
            if slots < n {
                return Err(ChunkError::TooFewSlots(slots));
            }
        }
        let head = b.collapse_position(head);
        Ok(ContiguousReadChunk {
            // SAFETY: ...
            ptr: unsafe { b.data_ptr().add(head) },
            len: n,
            consumer: self,
        })
    }
}
