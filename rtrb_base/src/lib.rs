#![no_std]
#![warn(rust_2018_idioms)]

use core::cell::{Cell, UnsafeCell};
use core::fmt;
use core::marker::PhantomData;
use core::mem::MaybeUninit;
use core::ops::Deref;
use core::sync::atomic::{AtomicUsize, Ordering};

// TODO: separate module for traits?
// TODO: separate module for "policies", maybe "diy"?

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

    #[inline(always)]
    fn update_capacity(capacity: usize) -> usize {
        capacity
    }

    fn collapse_position(pos: usize, capacity: usize) -> usize;

    /// Increments a position by going `n` slots forward.
    fn increment(pos: usize, n: usize, capacity: usize) -> usize;

    /// Increments a position by going one slot forward.
    ///
    /// This might be more efficient than self.increment(..., 1).
    #[inline]
    fn increment1(pos: usize, capacity: usize) -> usize {
        Self::increment(pos, 1, capacity)
    }

    /// Returns the distance between two positions.
    fn distance(a: usize, b: usize, capacity: usize) -> usize;
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

    fn capacity(&self) -> usize;

    fn indices(&self) -> &Self::Indices;

    #[inline(never)]
    fn drop_all_elements(&mut self) {
        let mut head = self.indices().head().load(Ordering::Relaxed);
        let tail = self.indices().tail().load(Ordering::Relaxed);

        // Loop over all slots that hold a value and drop them.
        while head != tail {
            // SAFETY: All slots between head and tail have been initialized.
            unsafe { self.slot_ptr(head).drop_in_place() };
            head = Self::Addr::increment1(head, self.capacity());
        }
        // This is not needed if drop_all_elements() is only called once,
        // but to be safe, we call it anyway:
        self.indices().head().store(head, Ordering::Relaxed);
    }

    /// Returns a pointer to the slot at position `pos`.
    ///
    /// If `pos == 0 && capacity == 0`, the returned pointer must not be dereferenced!
    #[inline]
    unsafe fn slot_ptr(&self, pos: usize) -> *mut Self::Item {
        self.data_ptr().add(Self::Addr::collapse_position(pos, self.capacity()))
    }

    //fn is_abandoned(this: &Self::Reference) -> bool;
}

#[derive(Debug, PartialEq, Eq)]
pub struct Producer<R> {
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
        Self {
            buffer,
            cached_head: Cell::new(0),
        }
    }

    pub fn push(&mut self, value: S::Item) -> Result<(), PushError<S::Item>> {
        if let Some(tail) = self.next_tail() {
            // SAFETY: tail points to an empty slot.
            unsafe { self.buffer.slot_ptr(tail).write(value) };
            let tail = S::Addr::increment1(tail, self.capacity());
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
        capacity - S::Addr::distance(head, tail, capacity)
    }

    pub fn is_full(&self) -> bool {
        self.next_tail().is_none()
    }

    #[inline(always)]
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
        // "tail" is only ever written by the producer thread, "Relaxed" is enough
        let tail = indices.tail().load(Ordering::Relaxed);
        let capacity = self.buffer.capacity();

        // Check if the queue is *possibly* full.
        if S::Addr::distance(self.cached_head.get(), tail, capacity) == capacity {
            // Refresh the head ...
            let head = indices.head().load(Ordering::Acquire);
            // ... and check if it's *really* full.
            if S::Addr::distance(head, tail, capacity) == capacity {
                return None;
            }
            self.cached_head.set(head);
        }
        Some(tail)
    }
}

#[derive(Debug, PartialEq, Eq)]
pub struct Consumer<R> {
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
        Self {
            buffer,
            cached_tail: Cell::new(0),
        }
    }

    pub fn pop(&mut self) -> Result<S::Item, PopError> {
        if let Some(head) = self.next_head() {
            // SAFETY: head points to an initialized slot.
            let value = unsafe { self.buffer.slot_ptr(head).read() };
            let head = S::Addr::increment1(head, self.capacity());
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
        S::Addr::distance(head, tail, self.capacity())
    }

    pub fn is_empty(&self) -> bool {
        self.next_head().is_none()
    }

    /*
    pub fn is_abandoned(&self) -> bool {
        S::is_abandoned(&self.buffer)
    }
    */

    #[inline(always)]
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
    _addr: PhantomData<A>,
    indices: I,

    /// The static array holding slots.
    ///
    /// This must be in an `UnsafeCell` because both producer and consumer
    /// have a (non-mutable) reference to the ring buffer and they use
    /// *interior mutability* to modify it.
    slots: UnsafeCell<[MaybeUninit<T>; N]>,

    /// Indicates that dropping a `StaticStorage` may drop elements of type `T`.
    _marker: PhantomData<T>,
}

impl<T, const N: usize, A: Addressing, I: Indices> StaticStorage<T, N, A, I> {
    #[must_use]
    pub fn new() -> Self {
        let capacity = A::update_capacity(N);
        // TODO: move this check to compile time!
        if capacity != N {
            panic!("StaticStorage doesn't support changing capacity");
        }
        Self {
            _addr: PhantomData,
            indices: I::new(),
            slots: UnsafeCell::new([const { MaybeUninit::uninit() }; N]),
            _marker: PhantomData,
        }
    }

    /// Split ...
    ///
    /// This takes a mutable reference, which makes sure that `split()` isn't called a second time.
    /// Holding a reference (regardless whether mutable or not) also guarantees that the storage
    /// isn't moved as long as a producer and consumer exist.
    pub fn split(&mut self) -> (Producer<&Self>, Consumer<&Self>) {
        // SAFETY: Only a single instance of Producer is allowed.
        let p = unsafe { Producer::new(&*self) };
        // SAFETY: Only a single instance of Consumer is allowed.
        let c = unsafe { Consumer::new(&*self) };
        (p, c)
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

    #[inline]
    fn data_ptr(&self) -> *mut Self::Item {
        // TODO: what happens if N == 0?
        self.slots.get().cast()
    }

    #[inline]
    fn capacity(&self) -> usize {
        N
    }

    #[inline]
    fn indices(&self) -> &Self::Indices {
        &self.indices
    }
}

/// Exact length.
#[derive(Debug)]
pub struct TightAddressing;

// SAFETY: all methods must be implemented correctly, or the whole thing is unsound
unsafe impl Addressing for TightAddressing {
    /// Wraps a position from the range `0 .. 2 * capacity` to `0 .. capacity`.
    #[inline]
    fn collapse_position(pos: usize, capacity: usize) -> usize {
        debug_assert!(pos == 0 || pos < 2 * capacity);
        if pos < capacity {
            pos
        } else {
            pos - capacity
        }
    }

    /// Increments a position by going `n` slots forward.
    #[inline]
    fn increment(pos: usize, n: usize, capacity: usize) -> usize {
        debug_assert!(pos == 0 || pos < 2 * capacity);
        debug_assert!(n <= capacity);
        let threshold = 2 * capacity - n;
        if pos < threshold {
            pos + n
        } else {
            pos - threshold
        }
    }

    #[inline]
    fn increment1(pos: usize, capacity: usize) -> usize {
        debug_assert_ne!(capacity, 0);
        debug_assert!(pos < 2 * capacity);
        if pos < 2 * capacity - 1 {
            pos + 1
        } else {
            0
        }
    }

    #[inline]
    fn distance(a: usize, b: usize, capacity: usize) -> usize {
        debug_assert!(a == 0 || a < 2 * capacity);
        debug_assert!(b == 0 || b < 2 * capacity);
        if a <= b {
            b - a
        } else {
            2 * capacity - a + b
        }
    }
}

/// Force power of two.
#[derive(Debug)]
pub struct PowerOfTwoAddressing;

/// The queue capacity is always a power of 2.
// SAFETY: all methods must be implemented correctly, or the whole thing is unsound
unsafe impl Addressing for PowerOfTwoAddressing {
    #[inline(always)]
    fn update_capacity(capacity: usize) -> usize {
        capacity.next_power_of_two()
    }

    #[inline]
    fn collapse_position(pos: usize, capacity: usize) -> usize {
        // TODO: is capacity 0 supported?
        pos & (capacity - 1)
    }

    #[inline]
    fn increment(pos: usize, n: usize, _capacity: usize) -> usize {
        pos.wrapping_add(n)
    }

    #[inline]
    fn distance(a: usize, b: usize, _capacity: usize) -> usize {
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
// TODO: change to newtype, add docs
pub type EmbeddedRingBuffer<T, const N: usize> = StaticStorage<T, N, TightAddressing, TightIndices>;
