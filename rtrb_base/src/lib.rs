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
    fn new() -> Self;

    fn head(&self) -> &AtomicUsize;
    fn tail(&self) -> &AtomicUsize;
}

/// Addressing.
///
/// # Safety
///
/// ...
pub unsafe trait Addressing {
    //type SizeType;
    // TODO: AtomicSizeType?

    fn new(capacity: usize) -> Self;

    fn capacity(&self) -> usize;

    fn collapse_position(&self, pos: usize) -> usize;

    /// Increments a position by going `n` slots forward.
    fn increment(&self, pos: usize, n: usize) -> usize;

    /// Increments a position by going one slot forward.
    ///
    /// This might be more efficient than self.increment(..., 1).
    #[inline]
    fn increment1(&self, pos: usize) -> usize {
        self.increment(pos, 1)
    }

    /// Returns the distance between two positions.
    fn distance(&self, a: usize, b: usize) -> usize;
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
    type Addr: Addressing;
    type Indices: Indices;

    //type Reference: Deref<Target = Self>;

    fn data_ptr(&self) -> *mut Self::Item;

    fn addr(&self) -> &Self::Addr;

    fn indices(&self) -> &Self::Indices;

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
            head = self.addr().increment1(head);
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
        self.data_ptr().add(self.addr().collapse_position(pos))
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
            let tail = self.buffer.addr().increment1(tail);
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
        self.buffer.addr().capacity() - self.buffer.addr().distance(head, tail)
    }

    pub fn is_full(&self) -> bool {
        self.next_tail().is_none()
    }

    pub fn capacity(&self) -> usize {
        self.buffer.addr().capacity()
    }

    /// Get the tail position for writing the next slot, if available.
    ///
    /// This is a strict subset of the functionality implemented in `write_chunk_uninit()`.
    /// For performance, this special case is immplemented separately.
    #[inline]
    fn next_tail(&self) -> Option<usize> {
        let indices = self.buffer.indices();
        let addr = self.buffer.addr();
        // "tail" is only ever written by the producer thread, "Relaxed" is enough
        let tail = indices.tail().load(Ordering::Relaxed);

        // Check if the queue is *possibly* full.
        if addr.distance(self.cached_head.get(), tail) == addr.capacity() {
            // Refresh the head ...
            let head = indices.head().load(Ordering::Acquire);
            // ... and check if it's *really* full.
            if addr.distance(head, tail) == addr.capacity() {
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
            let head = self.buffer.addr().increment1(head);
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
        self.buffer.addr().distance(head, tail)
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
        self.buffer.addr().capacity()
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
#[derive(Debug)]
pub struct StaticStorage<T, const N: usize, A: Addressing, I: Indices> {
    addr: A,
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
unsafe impl<T: Sync, const N: usize, A: Addressing + Sync, I: Indices + Sync> Sync for StaticStorage<T, N, A, I> {}

impl<T, const N: usize, A: Addressing, I: Indices> StaticStorage<T, N, A, I> {
    #[must_use]
    pub fn new() -> Self {
        let addr = A::new(N);
        // TODO: move this check to compile time!
        if addr.capacity() != N {
            panic!("StaticStorage doesn't support changing capacity");
        }
        Self {
            addr,
            indices: I::new(),
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

impl<T, const N: usize, A: Addressing, I: Indices> Drop for StaticStorage<T, N, A, I> {
    fn drop(&mut self) {
        // SAFETY: this is called exactly once, no references to any elements exist anymore.
        unsafe { self.drop_all_elements() };
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
unsafe impl<T, const N: usize, A: Addressing, I: Indices> Storage for StaticStorage<T, N, A, I> {
    type Item = T;
    type Addr = A;
    type Indices = I;

    #[inline(always)]
    fn data_ptr(&self) -> *mut Self::Item {
        // TODO: what happens if N == 0?
        self.slots.get().cast()
    }

    #[inline(always)]
    fn addr(&self) -> &Self::Addr {
        &self.addr
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

/// Exact length.
#[derive(Debug)]
pub struct TightAddressing {
    /// The queue capacity.
    capacity: usize,
}

// SAFETY: all methods must be implemented correctly, or the whole thing is unsound
unsafe impl Addressing for TightAddressing {
    fn new(capacity: usize) -> Self {
        Self { capacity }
    }

    #[inline]
    fn capacity(&self) -> usize {
        self.capacity
    }

    /// Wraps a position from the range `0 .. 2 * capacity` to `0 .. capacity`.
    #[inline]
    fn collapse_position(&self, pos: usize) -> usize {
        debug_assert!(pos == 0 || pos < 2 * self.capacity);
        if pos < self.capacity {
            pos
        } else {
            pos - self.capacity
        }
    }

    /// Increments a position by going `n` slots forward.
    #[inline]
    fn increment(&self, pos: usize, n: usize) -> usize {
        debug_assert!(pos == 0 || pos < 2 * self.capacity);
        debug_assert!(n <= self.capacity);
        let threshold = 2 * self.capacity - n;
        if pos < threshold {
            pos + n
        } else {
            pos - threshold
        }
    }

    #[inline]
    fn increment1(&self, pos: usize) -> usize {
        debug_assert_ne!(self.capacity, 0);
        debug_assert!(pos < 2 * self.capacity);
        if pos < 2 * self.capacity - 1 {
            pos + 1
        } else {
            0
        }
    }

    #[inline]
    fn distance(&self, a: usize, b: usize) -> usize {
        debug_assert!(a == 0 || a < 2 * self.capacity);
        debug_assert!(b == 0 || b < 2 * self.capacity);
        if a <= b {
            b - a
        } else {
            2 * self.capacity - a + b
        }
    }
}

/// Force power of two.
#[derive(Debug)]
pub struct PowerOfTwoAddressing {
    /// The queue capacity (a power of 2).
    capacity: usize,
}

// SAFETY: all methods must be implemented correctly, or the whole thing is unsound
unsafe impl Addressing for PowerOfTwoAddressing {
    fn new(capacity: usize) -> Self {
        Self {
            capacity: capacity.next_power_of_two(),
        }
    }

    #[inline]
    fn capacity(&self) -> usize {
        self.capacity
    }

    #[inline]
    fn collapse_position(&self, pos: usize) -> usize {
        // TODO: is capacity 0 supported?
        pos & (self.capacity - 1)
    }

    #[inline]
    fn increment(&self, pos: usize, n: usize) -> usize {
        pos.wrapping_add(n)
    }

    #[inline]
    fn distance(&self, a: usize, b: usize) -> usize {
        b.wrapping_sub(a)
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
    fn new() -> Self {
        TightIndices {
            head: AtomicUsize::new(0),
            tail: AtomicUsize::new(0),
        }
    }

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
/// TODO: move this to split() docs?
///
/// Only one pair of producer/consumer can exist at once:
/// ```compile_fail
/// # use rtrb_base::EmbeddedRingBuffer;
/// let mut rb = EmbeddedRingBuffer::<i32, 64>::new();
/// let (mut producer, mut consumer) = rb.split();
/// let (mut another_producer, mut another_consumer) = rb.split();
/// ```
/// TODO: show error message
///
/// Once both have been dropped, a new pair can be created:
/// ```
/// # use rtrb_base::EmbeddedRingBuffer;
/// let mut rb = EmbeddedRingBuffer::<i32, 64>::new();
/// let (mut producer, mut consumer) = rb.split();
/// drop(producer); drop(consumer);
/// let (mut another_producer, mut another_consumer) = rb.split();
/// ```
// TODO: change to newtype, add more docs
pub type EmbeddedRingBuffer<T, const N: usize> = StaticStorage<T, N, TightAddressing, TightIndices>;
