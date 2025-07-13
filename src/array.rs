//! ...
//!
//! no dynamic allocation, but cache-padded indices

use crate::{
    diy::{Calc, ArrayStorage},
    CachePaddedIndices, PeekError, PopError, PushError,
};

type Inner<T, const N: usize> = ArrayStorage<T, N, { Calc::DoubleSize as u8 }, CachePaddedIndices>;

#[derive(Debug)]
pub struct RingBuffer<T, const N: usize>(Inner<T, N>);

impl<T, const N: usize> RingBuffer<T, N> {
    pub const fn new() -> Self {
        Self(Inner::<T, N>::new())
    }

    pub fn producer(&self) -> Option<Producer<'_, T, N>> {
        self.0.producer().map(Producer)
    }

    pub fn consumer(&self) -> Option<Consumer<'_, T, N>> {
        self.0.consumer().map(Consumer)
    }
}

impl<T, const N: usize> Default for RingBuffer<T, N> {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, PartialEq, Eq)]
pub struct Producer<'a, T, const N: usize>(crate::diy::Producer<&'a Inner<T, N>>);

impl<T, const N: usize> Producer<'_, T, N> {
    pub fn push(&mut self, value: T) -> Result<(), PushError<T>> {
        self.0.push(value)
    }

    pub fn slots(&self) -> usize {
        self.0.slots()
    }

    pub fn is_full(&self) -> bool {
        self.0.is_full()
    }

    pub fn capacity(&self) -> usize {
        self.0.capacity()
    }

    /*
    pub fn is_abandoned(&self) -> bool {
        Arc::strong_count(&self.buffer) < 2
    }
    */
}

#[derive(Debug, PartialEq, Eq)]
pub struct Consumer<'a, T, const N: usize>(crate::diy::Consumer<&'a Inner<T, N>>);

impl<T, const N: usize> Consumer<'_, T, N> {
    pub fn pop(&mut self) -> Result<T, PopError> {
        self.0.pop()
    }

    pub fn peek(&self) -> Result<&T, PeekError> {
        self.0.peek()
    }

    pub fn slots(&self) -> usize {
        self.0.slots()
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /*
    pub fn is_abandoned(&self) -> bool {
        self.0.is_abandoned()
    }
    */

    pub fn capacity(&self) -> usize {
        self.0.capacity()
    }
}
