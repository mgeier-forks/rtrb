#![no_std]
#![warn(rust_2018_idioms)]

use core::cell::Cell;
use core::fmt;
use core::ops::Deref;
use core::sync::atomic::{AtomicUsize, Ordering};

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

    #[inline(never)]
    fn drop_all_elements(&mut self) {
        let mut head = self.indices().head().load(Ordering::Relaxed);
        let tail = self.indices().tail().load(Ordering::Relaxed);

        // Loop over all slots that hold a value and drop them.
        while head != tail {
            // SAFETY: All slots between head and tail have been initialized.
            unsafe { self.slot_ptr(head).drop_in_place() };
            head = self.addr().increment1(head);
        }
        // This is not needed if drop_all_elements() is only called once,
        // but to be safe, we call it anyway:
        self.indices().head().store(head, Ordering::Relaxed);
    }

    /// Returns a pointer to the slot at position `pos`.
    ///
    /// If `pos == 0 && capacity == 0`, the returned pointer must not be dereferenced!
    unsafe fn slot_ptr(&self, pos: usize) -> *mut Self::Item {
        self.data_ptr().add(self.addr().collapse_position(pos))
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

#[cfg(feature = "std")]
impl std::error::Error for PopError {}

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

#[cfg(feature = "std")]
impl std::error::Error for PeekError {}

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

#[cfg(feature = "std")]
impl<T> std::error::Error for PushError<T> {}

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
