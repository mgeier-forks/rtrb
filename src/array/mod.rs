// This file has been auto-generated ...


//! TODO: non-bip ring buffer.


/// A bounded single-producer single-consumer (SPSC) queue.
///
/// Elements can be written with a [`Producer`] and read with a [`Consumer`],
/// which can be obtained with ... TODO
///
/// *See also the [module-level documentation](rtrb::array).*
pub struct RingBuffer<T, const N: usize> {
}

impl<T, const N: usize> RingBuffer<T, N> {
    pub fn producer(&self) -> Option<Producer<'_, N>> {
    }
}
