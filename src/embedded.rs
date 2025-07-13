//! ...
//!
//! no cache padding, no dynamic allocation
//! power-of-two optimizations might be done automatically by the compiler? TODO: verify

use crate::diy::{Calc, ArrayStorage, TightIndices};

/// TODO: move this to producer()/consumer() docs?
///
/// Only one producer and one consumer can exist at once,
/// but once a producer/consumer has been dropped, a new one can be created:
/// ```
/// let rb = rtrb::embedded::RingBuffer::<i32, 64>::new();
/// let mut producer = rb.producer().unwrap();
/// let mut consumer = rb.consumer().unwrap();
/// assert!(rb.consumer().is_none());
/// assert_eq!(producer.push(10), Ok(()));
/// drop(producer);
/// let mut producer = rb.producer().unwrap();
/// assert_eq!(producer.push(20), Ok(()));
/// assert_eq!(consumer.pop(), Ok(10));
/// assert_eq!(consumer.pop(), Ok(20));
/// ```
// TODO: change to newtype, add more docs
pub type RingBuffer<T, const N: usize> =
    ArrayStorage<T, N, { Calc::DoubleSize as u8 }, TightIndices>;
