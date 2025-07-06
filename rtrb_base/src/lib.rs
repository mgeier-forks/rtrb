#![no_std]
#![warn(rust_2018_idioms)]

use core::cell::{Cell, UnsafeCell};
use core::fmt;
use core::mem::MaybeUninit;
use core::ops::Deref;
use core::sync::atomic::{AtomicU8, AtomicUsize, Ordering};

// TODO: separate module for traits?
// TODO: separate module for "policies", maybe "diy"?

const HAS_PRODUCER: u8 = 0b10000000;
const HAS_CONSUMER: u8 = 0b01000000;
// NB: This overlaps with HAS_PRODUCER, they are never used at the same time.
//const IS_ABANDONED: u8 = 0b10000000;

/// Indices.
///
/// # Safety
///
/// The indices must not be changed by anyone else.
pub unsafe trait Indices {
    const INIT: Self;

    fn head(&self) -> &AtomicUsize;
    fn tail(&self) -> &AtomicUsize;
}

/// Addressing.
#[repr(u8)]
pub enum Addressing {
    Tight,
    PowerOfTwo,
}

impl Addressing {
    // This is a work-around until the `adt_const_params` feature has been stabilized
    // (https://github.com/rust-lang/rust/issues/95174):
    pub const fn from_u8(value: u8) -> Addressing {
        if value == Addressing::Tight as u8 {
            Addressing::Tight
        } else if value == Addressing::PowerOfTwo as u8 {
            Addressing::PowerOfTwo
        } else {
            panic!("Invalid value for Addressing")
        }
    }

    pub const fn update_capacity(&self, x: usize) -> usize {
        // MSRV 1.46: match statements in const fn
        match self {
            Addressing::Tight => x,
            Addressing::PowerOfTwo => x.next_power_of_two(),
        }
    }

    #[inline]
    fn collapse_position(&self, pos: usize, capacity: usize) -> usize {
        match self {
            Addressing::Tight => {
                // Wraps a position from the range `0 .. 2 * capacity` to `0 .. capacity`.
                debug_assert!(pos == 0 || pos < 2 * capacity);
                if pos < capacity {
                    pos
                } else {
                    pos - capacity
                }
            }
            Addressing::PowerOfTwo => {
                // Wraps from any number to the range `0 .. capacity`.
                // TODO: is capacity 0 supported?
                pos & (capacity - 1)
            }
        }
    }

    /// Increments a position by going `n` slots forward.
    #[inline]
    fn increment(&self, pos: usize, n: usize, capacity: usize) -> usize {
        match self {
            Addressing::Tight => {
                debug_assert!(pos == 0 || pos < 2 * capacity);
                debug_assert!(n <= capacity);
                let threshold = 2 * capacity - n;
                if pos < threshold {
                    pos + n
                } else {
                    pos - threshold
                }
            }
            Addressing::PowerOfTwo => pos.wrapping_add(n),
        }
    }

    /// Increments a position by going one slot forward.
    ///
    /// This might be more efficient than self.increment(..., 1).
    #[inline]
    fn increment1(&self, pos: usize, capacity: usize) -> usize {
        match self {
            Addressing::Tight => {
                debug_assert_ne!(capacity, 0);
                debug_assert!(pos < 2 * capacity);
                if pos < 2 * capacity - 1 {
                    pos + 1
                } else {
                    0
                }
            }
            Addressing::PowerOfTwo => pos.wrapping_add(1),
        }
    }

    /// Returns the distance between two positions.
    #[inline]
    fn distance(&self, a: usize, b: usize, capacity: usize) -> usize {
        match self {
            Addressing::Tight => {
                debug_assert!(a == 0 || a < 2 * capacity);
                debug_assert!(b == 0 || b < 2 * capacity);
                if a <= b {
                    b - a
                } else {
                    2 * capacity - a + b
                }
            }
            Addressing::PowerOfTwo => b.wrapping_sub(a),
        }
    }
}

/// Storage.
///
/// # Safety
///
/// Storage must be contiguous.
///
/// ...
pub unsafe trait Storage {
    type Item;
    type Indices: Indices;
    const ADDR: Addressing;

    fn data_ptr(&self) -> *mut Self::Item;

    fn indices(&self) -> &Self::Indices;

    fn capacity(&self) -> usize;

    /// Do whatever is needed when the `Producer` is dropped.
    ///
    /// # Safety
    ///
    /// This can only be called in `Producer::drop()`.
    unsafe fn drop_producer(&self) {}

    /// Do whatever is needed when the `Consumer` is dropped.
    ///
    /// # Safety
    ///
    /// This can only be called in `Consumer::drop()`.
    unsafe fn drop_consumer(&self) {}

    /// Drop all elements that are still in the buffer.
    ///
    /// After this, head and tail indices are invalid.
    ///
    /// # Safety
    ///
    /// This can only be called in the `Drop` implementation of the storage.
    #[inline(never)]
    unsafe fn drop_all_elements(&mut self) {
        let mut head = self.indices().head().load(Ordering::Relaxed);
        let tail = self.indices().tail().load(Ordering::Relaxed);

        // Loop over all slots that hold a value and drop them.
        while head != tail {
            // SAFETY: All slots between head and tail have been initialized.
            unsafe { self.slot_ptr(head).drop_in_place() };
            head = Self::ADDR.increment1(head, self.capacity());
        }
    }

    /// Returns a pointer to the (possibly uninitialized) slot at position `pos`.
    ///
    /// # Safety
    ///
    /// `pos` must be valid.
    ///
    /// If `pos == 0 && capacity == 0`, the returned pointer must not be dereferenced!
    #[inline]
    unsafe fn slot_ptr(&self, pos: usize) -> *mut Self::Item {
        self.data_ptr()
            .add(Self::ADDR.collapse_position(pos, self.capacity()))
    }
}

#[derive(Debug, PartialEq, Eq)]
// NB: this syntax needs MSRV 1.79
//pub struct Producer<R: Deref<Target: Storage>>
pub struct Producer<R: Deref>
where
    <R as Deref>::Target: Storage,
{
    /// A reference to the ring buffer.
    buffer: R,

    /// A copy of `buffer.head` for quick access.
    ///
    /// This value can be stale and sometimes needs to be resynchronized with `buffer.head`.
    cached_head: Cell<usize>,
}

// SAFETY: After moving a Producer to another thread, there is still only a single thread
// that can access the producer side of the queue.
unsafe impl<S: Storage, R: Deref<Target = S>> Send for Producer<R> where S::Item: Send {}

impl<S: Storage, R: Deref<Target = S>> Producer<R> {
    #[doc(hidden)]
    pub unsafe fn new(buffer: R) -> Self {
        let head = buffer.indices().head().load(Ordering::Acquire);
        Self {
            buffer,
            cached_head: Cell::new(head),
        }
    }

    pub fn push(&mut self, value: S::Item) -> Result<(), PushError<S::Item>> {
        if let Some(tail) = self.next_tail() {
            // SAFETY: tail points to an empty slot.
            unsafe { self.buffer.slot_ptr(tail).write(value) };
            let tail = S::ADDR.increment1(tail, self.buffer.capacity());
            self.buffer.indices().tail().store(tail, Ordering::Release);
            Ok(())
        } else {
            Err(PushError::Full(value))
        }
    }

    pub fn slots(&self) -> usize {
        let head = self.buffer.indices().head().load(Ordering::Acquire);
        self.cached_head.set(head);
        // "tail" is only ever written by the producer thread, "Relaxed" is enough
        let tail = self.buffer.indices().tail().load(Ordering::Relaxed);
        let capacity = self.buffer.capacity();
        capacity - S::ADDR.distance(head, tail, capacity)
    }

    pub fn is_full(&self) -> bool {
        self.next_tail().is_none()
    }

    pub fn capacity(&self) -> usize {
        self.buffer.capacity()
    }

    /// Get the tail position for writing the next slot, if available.
    ///
    /// This is a strict subset of the functionality implemented in `write_chunk_uninit()`.
    /// For performance, this special case is immplemented separately.
    #[inline]
    fn next_tail(&self) -> Option<usize> {
        let indices = self.buffer.indices();
        let capacity = self.buffer.capacity();
        // "tail" is only ever written by the producer thread, "Relaxed" is enough
        let tail = indices.tail().load(Ordering::Relaxed);

        // Check if the queue is *possibly* full.
        if S::ADDR.distance(self.cached_head.get(), tail, capacity) == capacity {
            // Refresh the head ...
            let head = indices.head().load(Ordering::Acquire);
            // ... and check if it's *really* full.
            if S::ADDR.distance(head, tail, capacity) == capacity {
                return None;
            }
            self.cached_head.set(head);
        }
        Some(tail)
    }
}

impl<R: Deref> Drop for Producer<R>
where
    <R as Deref>::Target: Storage,
{
    fn drop(&mut self) {
        // SAFETY: This is only called in Producer::drop().
        unsafe { self.buffer.drop_producer() };
    }
}

#[derive(Debug, PartialEq, Eq)]
pub struct Consumer<R: Deref>
where
    <R as Deref>::Target: Storage,
{
    /// A reference to the ring buffer.
    buffer: R,

    /// A copy of `buffer.tail` for quick access.
    ///
    /// This value can be stale and sometimes needs to be resynchronized with `buffer.tail`.
    cached_tail: Cell<usize>,
}

// SAFETY: After moving a Consumer to another thread, there is still only a single thread
// that can access the consumer side of the queue.
unsafe impl<S: Storage, R: Deref<Target = S>> Send for Consumer<R> where S::Item: Send {}

impl<S: Storage, R: Deref<Target = S>> Consumer<R> {
    #[doc(hidden)]
    pub unsafe fn new(buffer: R) -> Self {
        let tail = buffer.indices().tail().load(Ordering::Acquire);
        Self {
            buffer,
            cached_tail: Cell::new(tail),
        }
    }

    pub fn pop(&mut self) -> Result<S::Item, PopError> {
        if let Some(head) = self.next_head() {
            // SAFETY: head points to an initialized slot.
            let value = unsafe { self.buffer.slot_ptr(head).read() };
            let head = S::ADDR.increment1(head, self.buffer.capacity());
            self.buffer.indices().head().store(head, Ordering::Release);
            Ok(value)
        } else {
            Err(PopError::Empty)
        }
    }

    pub fn peek(&self) -> Result<&S::Item, PeekError> {
        if let Some(head) = self.next_head() {
            // SAFETY: head points to an initialized slot.
            Ok(unsafe { &*self.buffer.slot_ptr(head) })
        } else {
            Err(PeekError::Empty)
        }
    }

    pub fn slots(&self) -> usize {
        let tail = self.buffer.indices().tail().load(Ordering::Acquire);
        self.cached_tail.set(tail);
        // "head" is only ever written by the consumer thread, "Relaxed" is enough
        let head = self.buffer.indices().head().load(Ordering::Relaxed);
        S::ADDR.distance(head, tail, self.buffer.capacity())
    }

    pub fn is_empty(&self) -> bool {
        self.next_head().is_none()
    }

    /*
    pub fn is_abandoned(&self) -> bool {
        S::is_abandoned(&self.buffer)
    }
    */

    pub fn capacity(&self) -> usize {
        self.buffer.capacity()
    }

    /// Get the head position for reading the next slot, if available.
    ///
    /// This is a strict subset of the functionality implemented in `read_chunk()`.
    /// For performance, this special case is immplemented separately.
    #[inline]
    fn next_head(&self) -> Option<usize> {
        let indices = self.buffer.indices();
        // "head" is only ever written by the consumer thread, "Relaxed" is enough
        let head = indices.head().load(Ordering::Relaxed);

        // Check if the queue is *possibly* empty.
        if head == self.cached_tail.get() {
            // Refresh the tail ...
            let tail = indices.tail().load(Ordering::Acquire);
            // ... and check if it's *really* empty.
            if head == tail {
                return None;
            }
            self.cached_tail.set(tail);
        }
        Some(head)
    }
}

impl<R: Deref> Drop for Consumer<R>
where
    <R as Deref>::Target: Storage,
{
    fn drop(&mut self) {
        // SAFETY: This is only called in Consumer::drop().
        unsafe { self.buffer.drop_consumer() };
    }
}

/// Error type for [`Consumer::pop()`].
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum PopError {
    /// The queue was empty.
    Empty,
}

/*
#[cfg(feature = "std")]
impl std::error::Error for PopError {}
*/

impl fmt::Display for PopError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PopError::Empty => "empty ring buffer".fmt(f),
        }
    }
}

/// Error type for [`Consumer::peek()`].
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum PeekError {
    /// The queue was empty.
    Empty,
}

/*
#[cfg(feature = "std")]
impl std::error::Error for PeekError {}
*/

impl fmt::Display for PeekError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PeekError::Empty => "empty ring buffer".fmt(f),
        }
    }
}

/// Error type for [`Producer::push()`].
#[derive(Copy, Clone, PartialEq, Eq)]
pub enum PushError<T> {
    /// The queue was full.
    Full(T),
}

/*
#[cfg(feature = "std")]
impl<T> std::error::Error for PushError<T> {}
*/

impl<T> fmt::Debug for PushError<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PushError::Full(_) => f.pad("Full(_)"),
        }
    }
}

impl<T> fmt::Display for PushError<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PushError::Full(_) => "full ring buffer".fmt(f),
        }
    }
}

/// Static storage.
// Once the `adt_const_params` feature has been stabilized
// (https://github.com/rust-lang/rust/issues/95174),
// `u8` can be replaced by `Addressing`.
#[derive(Debug)]
pub struct StaticStorage<T, const N: usize, const A: u8, I: Indices> {
    indices: I,

    /// Indicates whether a producer and/or a consumer is connected.
    flags: AtomicU8,

    /// The static array holding slots.
    ///
    /// This must be in an `UnsafeCell` because both producer and consumer
    /// have a (non-mutable) reference to the ring buffer and they use
    /// *interior mutability* to modify it.
    slots: UnsafeCell<[MaybeUninit<T>; N]>,
}

// TODO: check if this is correct:
unsafe impl<T: Sync, const N: usize, const A: u8, I: Indices + Sync> Sync
    for StaticStorage<T, N, A, I>
{
}

impl<T, const N: usize, const A: u8, I: Indices> StaticStorage<T, N, A, I> {
    #[must_use]
    pub const fn new() -> Self {
        const {
            assert!(
                N == Addressing::from_u8(A).update_capacity(N),
                "StaticStorage doesn't support changing capacity"
            );
        }
        Self {
            indices: I::INIT,
            flags: AtomicU8::new(0),
            slots: UnsafeCell::new([const { MaybeUninit::uninit() }; N]),
        }
    }

    pub fn producer(&self) -> Option<Producer<&Self>> {
        let old_flags = self.flags.fetch_or(HAS_PRODUCER, Ordering::SeqCst);
        if old_flags & HAS_PRODUCER == 0 {
            // SAFETY: This is the one and only producer.
            Some(unsafe { Producer::new(self) })
        } else {
            None
        }
    }

    pub fn consumer(&self) -> Option<Consumer<&Self>> {
        let old_flags = self.flags.fetch_or(HAS_CONSUMER, Ordering::SeqCst);
        if old_flags & HAS_CONSUMER == 0 {
            // SAFETY: This is the one and only consumer.
            Some(unsafe { Consumer::new(self) })
        } else {
            None
        }
    }
}

impl<T, const N: usize, const A: u8, I: Indices> Drop for StaticStorage<T, N, A, I> {
    fn drop(&mut self) {
        // SAFETY: this is called exactly once, no references to any elements exist anymore.
        unsafe { self.drop_all_elements() };
    }
}

impl<T, const N: usize, const A: u8, I: Indices> Default for StaticStorage<T, N, A, I> {
    fn default() -> Self {
        Self::new()
    }
}

/*
impl<T, A: Addressing, I: Indices> PartialEq for StaticStorage<T, A, I> {
    fn eq(&self, other: &Self) -> bool {
        core::ptr::eq(self, other)
    }
}

impl<T, A: Addressing, I: Indices> Eq for StaticStorage<T, A, I> {}
*/

// SAFETY: all methods must be implemented correctly, or the whole thing is unsound
unsafe impl<T, const N: usize, const A: u8, I: Indices> Storage for StaticStorage<T, N, A, I> {
    type Item = T;
    type Indices = I;
    const ADDR: Addressing = Addressing::from_u8(A);

    #[inline(always)]
    fn data_ptr(&self) -> *mut Self::Item {
        // TODO: what happens if N == 0?
        self.slots.get().cast()
    }

    #[inline(always)]
    fn capacity(&self) -> usize {
        N
    }
    #[inline(always)]
    fn indices(&self) -> &Self::Indices {
        &self.indices
    }

    #[inline(always)]
    unsafe fn drop_producer(&self) {
        let _ = self.flags.fetch_and(!HAS_PRODUCER, Ordering::SeqCst);
    }

    #[inline(always)]
    unsafe fn drop_consumer(&self) {
        let _ = self.flags.fetch_and(!HAS_CONSUMER, Ordering::SeqCst);
    }
}

/// Unpadded indices.
// TODO: generic argument for size type?
#[derive(Debug)]
pub struct TightIndices {
    /// The head of the queue.
    ///
    /// This integer is in range `0 .. 2 * capacity`.
    head: AtomicUsize,

    /// The tail of the queue.
    ///
    /// This integer is in range `0 .. 2 * capacity`.
    tail: AtomicUsize,
}

// SAFETY: all methods must be implemented correctly, or the whole thing is unsound
unsafe impl Indices for TightIndices {
    #[allow(clippy::declare_interior_mutable_const)]
    const INIT: Self = TightIndices {
        head: AtomicUsize::new(0),
        tail: AtomicUsize::new(0),
    };

    #[inline]
    fn head(&self) -> &AtomicUsize {
        &self.head
    }

    #[inline]
    fn tail(&self) -> &AtomicUsize {
        &self.tail
    }
}

/// ...
///
/// no cache padding, no dynamic allocation
/// power-of-two optimizations might be done automatically by the compiler? TODO: verify
///
/// TODO: move this to producer()/consumer() docs?
///
/// Only one producer and one consumer can exist at once,
/// but once a producer/consumer has been dropped, a new one can be created:
/// ```
/// # use rtrb_base::EmbeddedRingBuffer;
/// let rb = EmbeddedRingBuffer::<i32, 64>::new();
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
pub type EmbeddedRingBuffer<T, const N: usize> =
    StaticStorage<T, N, { Addressing::Tight as u8 }, TightIndices>;
pub type BrokenRingBuffer<T, const N: usize> = StaticStorage<T, N, 77, TightIndices>;
