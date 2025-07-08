//! ...
//!
//! no dynamic allocation, but cache-padded indices

use crate::{
    diy::{Addressing, ArrayStorage},
    CachePaddedIndices, PeekError, PopError, PushError,
};

type Inner<T, const N: usize> = ArrayStorage<T, N, { Addressing::Tight as u8 }, CachePaddedIndices>;

#[derive(Debug)]
pub struct RingBuffer<T, const N: usize>(Inner<T, N>);

impl<T, const N: usize> RingBuffer<T, N> {
    #[inline(always)]
    pub const fn new() -> Self {
        Self(Inner::<T, N>::new())
    }

    #[inline(always)]
    pub fn producer(&self) -> Option<Producer<'_, T, N>> {
        self.0.producer().map(Producer)
    }

    #[inline(always)]
    pub fn consumer(&self) -> Option<Consumer<'_, T, N>> {
        self.0.consumer().map(Consumer)
    }
}

impl<T, const N: usize> Default for RingBuffer<T, N> {
    #[inline(always)]
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, PartialEq, Eq)]
pub struct Producer<'a, T, const N: usize>(crate::diy::Producer<&'a Inner<T, N>>);

impl<T, const N: usize> Producer<'_, T, N> {
    #[inline(always)]
    pub fn push(&mut self, value: T) -> Result<(), PushError<T>> {
        self.0.push(value)
    }

    #[inline(always)]
    pub fn slots(&self) -> usize {
        self.0.slots()
    }

    #[inline(always)]
    pub fn is_full(&self) -> bool {
        self.0.is_full()
    }

    #[inline(always)]
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
    #[inline(always)]
    pub fn pop(&mut self) -> Result<T, PopError> {
        self.0.pop()
    }

    #[inline(always)]
    pub fn peek(&self) -> Result<&T, PeekError> {
        self.0.peek()
    }

    #[inline(always)]
    pub fn slots(&self) -> usize {
        self.0.slots()
    }

    #[inline(always)]
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /*
    #[inline(always)]
    pub fn is_abandoned(&self) -> bool {
        self.0.is_abandoned()
    }
    */

    #[inline(always)]
    pub fn capacity(&self) -> usize {
        self.0.capacity()
    }
}
