//! A bi-partite ring buffer.
//!
//! Simon Cooke (2003)
//! https://www.codeproject.com/Articles/3479/The-Bip-Buffer-The-Circular-Buffer-with-a-Twist
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
//! https://ferrous-systems.com/blog/lock-free-ring-buffer/
//! https://blog.systems.ethz.ch/blog/2019/the-design-and-implementation-of-a-lock-free-ring-buffer-with-contiguous-reservations.html
//!
//! Other Rust implementations:
//! https://crates.io/crates/bbqueue
//! https://crates.io/crates/bipbuffer (not thread-safe)
//! https://crates.io/crates/spsc-bip-buffer
//!
//! Implementations in other languages:
//! https://github.com/willemt/bipbuffer (C)

use core::{
    cell::Cell,
    mem::ManuallyDrop,
    ptr::NonNull,
    sync::atomic::{AtomicU8, AtomicUsize, Ordering},
};

use crate::{cache_padded::CachePadded, diy::IS_ABANDONED, PeekError, PopError, PushError};

/// Bi-partite ring buffer.
#[derive(Debug)]
pub struct RingBuffer<T> {
    head: CachePadded<AtomicUsize>,
    tail: CachePadded<AtomicUsize>,
    // TODO: 4-letter word? brim, line, clip, jump, muck, bump, tack, skip, trim, crop, wrap, high
    // TODO: measure whether CachePadded helps
    unused: CachePadded<AtomicUsize>,
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
            head: CachePadded::new(AtomicUsize::new(0)),
            tail: CachePadded::new(AtomicUsize::new(0)),
            unused: CachePadded::new(AtomicUsize::new(capacity)),
            flags: AtomicU8::new(0),
            data_ptr: ManuallyDrop::new(Vec::with_capacity(capacity)).as_mut_ptr(),
            capacity,
        })
    }

    // IndexCalculation

    fn capacity(&self) -> usize {
        self.capacity
    }

    fn data_ptr(&self) -> *mut T {
        self.data_ptr
    }

    fn flags(&self) -> &AtomicU8 {
        &self.flags
    }
}

// SAFETY: ...
unsafe impl<T: Send> Sync for RingBuffer<T> {}

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
    // TODO: cached_unused?
}

impl<T> Producer<T> {
    pub fn push(&mut self, value: T) -> Result<(), PushError<T>> {
        todo!()
    }
    pub fn slots(&self) -> usize {
        todo!()
    }
    pub fn is_full(&self) -> bool {
        todo!()
    }
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
    // TODO: cached_unused?
}

impl<T> Consumer<T> {
    pub fn pop(&mut self) -> Result<T, PopError> {
        todo!()
    }
    pub fn peek(&self) -> Result<&T, PeekError> {
        todo!()
    }
    pub fn slots(&self) -> usize {
        todo!()
    }
    pub fn is_empty(&self) -> bool {
        todo!()
    }
    pub fn is_abandoned(&self) -> bool {
        todo!()
    }
    pub fn capacity(&self) -> usize {
        todo!()
    }
}

/// Non-public helper type.
//#[derive(Debug, PartialEq, Eq)]
struct BoxedRingBuffer<T> {
    ptr: NonNull<RingBuffer<T>>,
}

impl<T> BoxedRingBuffer<T> {
    // NB: This takes ownership of the ring buffer, which makes sure that there is only one
    // Producer/Consumer in the end.
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
        let flags: &AtomicU8 = unsafe { self.ptr.as_ref().flags() };
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
        // SAFETY: There are no mutable references.
        unsafe { self.ptr.as_ref() }
    }
}
