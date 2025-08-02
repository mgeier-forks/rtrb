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

use crate::{chunks::ChunkError, diy::IS_ABANDONED, PeekError, PopError, PushError};

storage_vec! {
    padded = yes,
    bip = yes,
    rb_doc = "
Bi-partite ring buffer.

TODO: some more docs, maybe links? [`RingBuffer::new()`].

*See also the [module-level documentation](crate::bip).*
"
}

impl_everything_eventually! {
    bip = yes,
    pow2 = no,
    'a = (),
    N = ()
}

def_producer_consumer_arc! {}

def_arc_ring_buffer! {}

impl_producer_consumer_bip! {
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
