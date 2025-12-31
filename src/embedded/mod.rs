// This file has been auto-generated ...


//! TODO: non-bip ring buffer.

//! TODO: embedded
//!
//! no cache padding, no dynamic allocation
//! power-of-two optimizations might be done automatically by the compiler? TODO: verify

/// A bounded single-producer single-consumer (SPSC) queue.
///
/// Elements can be written with a [`Producer`] and read with a [`Consumer`],
/// which can be obtained with ... TODO
///
/// *See also the [module-level documentation](rtrb::embedded).*
pub struct RingBuffer<T, const N: usize> {
}

impl<T, const N: usize> RingBuffer<T, N> {
    pub fn producer(&self) -> Option<Producer<'_, N>> {
    }
}
