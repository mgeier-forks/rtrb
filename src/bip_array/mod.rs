// This file has been auto-generated ...


//! A bi-partite ring buffer whose elements are stored in an [array].
//!
//! See [`rtrb::bip_arc`](crate::bip_arc) for a bi-partite ring buffer with dynamic storage.
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


/// A bounded single-producer single-consumer (SPSC) queue.
///
/// Elements can be written with a [`Producer`] and read with a [`Consumer`],
/// which can be obtained with ... TODO
///
/// *See also the [module-level documentation](rtrb::bip_array).*
pub struct RingBuffer<T, const N: usize> {
}

impl<T, const N: usize> RingBuffer<T, N> {
    pub fn producer(&self) -> Option<Producer<'_, N>> {
    }
}
