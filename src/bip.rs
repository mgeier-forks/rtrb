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
//! Rust implementations:
//! https://crates.io/crates/bipbuffer (not thread-safe)
//! https://crates.io/crates/spsc-bip-buffer

use core::{marker::PhantomData, mem::ManuallyDrop, sync::atomic::AtomicU8};

use crate::{diy::{Calc, IndexCalculation, Indices, Ptr, Storage}, CachePaddedIndices, PeekError, PopError, PushError};

type Inner<T> = BipStorage<T, { Calc::DoubleSize as u8 }, CachePaddedIndices>;

/// Bi-partite ring buffer.
#[derive(Debug)]
pub struct RingBuffer<T>(PhantomData<Inner<T>>);

impl<T> RingBuffer<T> {
    #[allow(clippy::new_ret_no_self)]
    pub fn new(capacity: usize) -> (Producer<T>, Consumer<T>) {
        let (p, c) = Inner::<T>::new(capacity);
        (Producer(p), Consumer(c))
    }
}

// TODO: consolidate into DynamicStorage?
#[derive(Debug)]
pub struct BipStorage<T, const C: u8, I: Indices> {
    indices: I,

    flags: AtomicU8,

    data_ptr: *mut T,

    capacity: usize,
}

// SAFETY: ...
unsafe impl<T: Send, const C: u8, I: Indices + Sync> Sync for BipStorage<T, C, I> {}

impl<T, const C: u8, I: Indices> BipStorage<T, C, I> {
    #[allow(clippy::new_ret_no_self, clippy::type_complexity)]
    pub fn new(
        capacity: usize,
    ) -> (
        crate::diy::Producer<Ptr<BipStorage<T, C, I>>>,
        crate::diy::Consumer<Ptr<BipStorage<T, C, I>>>,
    ) {
        let capacity = Calc::from_u8(C).update_capacity(capacity);
        Ptr::new(Self {
            indices: I::INIT,
            flags: AtomicU8::new(0),
            data_ptr: ManuallyDrop::new(Vec::with_capacity(capacity)).as_mut_ptr(),
            capacity,
        })
    }
}

impl<T, const C: u8, I: Indices> PartialEq for BipStorage<T, C, I> {
    fn eq(&self, other: &Self) -> bool {
        core::ptr::eq(self, other)
    }
}

impl<T, const C: u8, I: Indices> Eq for BipStorage<T, C, I> {}

impl<T, const C: u8, I: Indices> IndexCalculation for BipStorage<T, C, I> {
    const CALC: Calc = Calc::from_u8(C);

    fn capacity(&self) -> usize {
        self.capacity
    }
}

// SAFETY: all methods must be implemented correctly, or the whole thing is unsound
unsafe impl<T, const C: u8, I: Indices> Storage for BipStorage<T, C, I> {
    type Item = T;
    type Indices = I;

    fn data_ptr(&self) -> *mut Self::Item {
        self.data_ptr
    }

    fn indices(&self) -> &Self::Indices {
        &self.indices
    }

    fn flags(&self) -> &AtomicU8 {
        &self.flags
    }
}

#[derive(Debug, PartialEq, Eq)]
pub struct Producer<T>(crate::diy::Producer<Ptr<Inner<T>>>);


impl<T> Producer<T> {
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
    pub fn is_abandoned(&self) -> bool {
        self.0.is_abandoned()
    }
}
#[derive(Debug, PartialEq, Eq)]
pub struct Consumer<T>(crate::diy::Consumer<Ptr<Inner<T>>>);

impl<T> Consumer<T> {
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
    pub fn is_abandoned(&self) -> bool {
        self.0.is_abandoned()
    }
    pub fn capacity(&self) -> usize {
        self.0.capacity()
    }
}
