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
    arc = yes,
    bip = yes,
    contiguous = yes,
    pow2 = no,
    'a = (),
    N = ()
}

impl<T> Producer<T> {
    /// The maximum number of slots that write_chunk() ... can provide.
    ///
    /// ... this can change at any time, up to ..., depending on ...
    pub fn slots_contiguous(&self) -> usize {
        todo!()
    }
    // TODO: return pair? without skipping, after skipping
    // TODO: is it cheaper to return a pair instead of calling 2 functions?
    pub fn slots_contiguous1(&self) -> usize {
        todo!()
    }
    pub fn slots_contiguous_first(&self) -> usize {
        todo!()
    }
    pub fn slots_without_wrapping(&self) -> usize {
        todo!()
    }
    pub fn slots_without_wrap_around(&self) -> usize {
        todo!()
    }
    pub fn slots_without_skipping(&self) -> usize {
        todo!()
    }
    pub fn is_abandoned(&self) -> bool {
        todo!()
    }
}
