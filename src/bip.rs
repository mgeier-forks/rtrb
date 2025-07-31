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
// TODO: move somewhere else
pub const NO_SKIP: usize = 0;

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

// TODO: bip option?
def_producer_consumer_boxed! {}

impl_producer_consumer_common! {
    'a = (),
    N = ()
}

impl_producer_consumer_bip! {
    'a = (),
    N = ()
}

impl_next_head_bip! {
    'a = (),
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

impl<T> Producer<T> {
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

impl<T> Consumer<T> {
    pub fn peek(&self) -> Result<&T, PeekError> {
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

// "chunks" stuff.

impl_chunks_bip! {
    'a = (),
    N = ()
}
impl_chunks_contiguous! {
    'a = (),
    N = ()
}
impl_chunks_common! {
    N = ()
}
