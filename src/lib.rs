//! A realtime-safe single-producer single-consumer ring buffer.
//!
//! Reading from and writing into the ring buffer is lock-free and wait-free.
//! All reading and writing functions return immediately.
//! Only a single thread can write into the ring buffer and a single thread
//! (typically a different one) can read from the ring buffer.
//! If the queue is empty, there is no way for the reading thread to wait
//! for new data, other than trying repeatedly until reading succeeds.
//! Similarly, if the queue is full, there is no way for the writing thread
//! to wait for newly available space to write to, other than trying repeatedly.
//!
//! A [`RingBuffer`] consists of two parts:
//! a [`Producer`] for writing into the ring buffer and
//! a [`Consumer`] for reading from the ring buffer.
//!
//! # Examples
//!
//! ```
//! use rtrb::RingBuffer;
//!
//! let (mut p, mut c) = RingBuffer::new(2).split();
//!
//! assert!(p.push(1).is_ok());
//! assert!(p.push(2).is_ok());
//! assert!(p.push(3).is_err());
//!
//! assert_eq!(c.pop(), Ok(1));
//! assert_eq!(c.pop(), Ok(2));
//! assert!(c.pop().is_err());
//! ```

#![warn(rust_2018_idioms)]
#![deny(missing_docs)]

use std::cell::Cell;
use std::fmt;
use std::marker::PhantomData;
use std::mem;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

use cache_padded::CachePadded;

mod error;

pub use error::{PeekError, PopError, PushError, SlicesError};

/// A bounded single-producer single-consumer queue.
pub struct RingBuffer<T> {
    /// The head of the queue.
    ///
    /// This integer is in range `0 .. 2 * capacity`.
    head: CachePadded<AtomicUsize>,

    /// The tail of the queue.
    ///
    /// This integer is in range `0 .. 2 * capacity`.
    tail: CachePadded<AtomicUsize>,

    /// The buffer holding slots.
    buffer: *mut T,

    /// The queue capacity.
    capacity: usize,

    /// Indicates that dropping a `Buffer<T>` may drop elements of type `T`.
    _marker: PhantomData<T>,
}

impl<T> RingBuffer<T> {
    /// Creates a [`RingBuffer`] with the given capacity.
    ///
    /// The returned [`RingBuffer`] is typically immediately split into
    /// the producer and the consumer side by [`RingBuffer::split()`].
    ///
    /// # Panics
    ///
    /// Panics if the capacity is zero.
    ///
    /// # Examples
    ///
    /// ```
    /// use rtrb::RingBuffer;
    ///
    /// let rb = RingBuffer::<f32>::new(100);
    /// ```
    /// Specifying an explicit type with the [turbofish](https://turbo.fish/)
    /// is is only necessary if it cannot be deduced by the compiler.
    /// ```
    /// use rtrb::RingBuffer;
    ///
    /// let (mut p, c) = RingBuffer::new(100).split();
    /// assert!(p.push(0.0f32).is_ok());
    /// ```
    pub fn new(capacity: usize) -> RingBuffer<T> {
        assert!(capacity > 0, "capacity must be non-zero");

        // Allocate a buffer of length `capacity`.
        let buffer = {
            let mut v = Vec::<T>::with_capacity(capacity);
            let ptr = v.as_mut_ptr();
            mem::forget(v);
            ptr
        };
        RingBuffer {
            head: CachePadded::new(AtomicUsize::new(0)),
            tail: CachePadded::new(AtomicUsize::new(0)),
            buffer,
            capacity,
            _marker: PhantomData,
        }
    }

    /// Splits the [`RingBuffer`] into [`Producer`] and [`Consumer`].
    ///
    /// # Examples
    ///
    /// ```
    /// use rtrb::RingBuffer;
    ///
    /// let (p, c) = RingBuffer::<f32>::new(100).split();
    /// ```
    pub fn split(self) -> (Producer<T>, Consumer<T>) {
        let rb = Arc::new(self);
        let p = Producer {
            rb: rb.clone(),
            head: Cell::new(0),
            tail: Cell::new(0),
            initialized: 0,
        };
        let c = Consumer {
            rb,
            head: Cell::new(0),
            tail: Cell::new(0),
        };
        (p, c)
    }

    /// Returns the capacity of the queue.
    ///
    /// # Examples
    ///
    /// ```
    /// use rtrb::RingBuffer;
    ///
    /// let rb = RingBuffer::<f32>::new(100);
    /// assert_eq!(rb.capacity(), 100);
    /// ```
    pub fn capacity(&self) -> usize {
        self.capacity
    }

    /// Returns a pointer to the slot at position `pos`.
    ///
    /// The position must be in range `0 .. 2 * capacity`.
    unsafe fn slot(&self, pos: usize) -> *mut T {
        if pos < self.capacity {
            self.buffer.add(pos)
        } else {
            self.buffer.add(pos - self.capacity)
        }
    }

    /// Increments a position by going `n` slots forward.
    ///
    /// The position must be in range `0 .. 2 * capacity`.
    fn increment(&self, pos: usize, n: usize) -> usize {
        let threshold = 2 * self.capacity - n;
        if pos < threshold {
            pos + n
        } else {
            pos - threshold
        }
    }

    /// Increments a position by going one slot forward.
    ///
    /// This is more efficient than self.increment(..., 1).
    ///
    /// The position must be in range `0 .. 2 * capacity`.
    fn increment1(&self, pos: usize) -> usize {
        if pos < 2 * self.capacity - 1 {
            pos + 1
        } else {
            0
        }
    }

    /// Returns the distance between two positions.
    ///
    /// Positions must be in range `0 .. 2 * capacity`.
    fn distance(&self, a: usize, b: usize) -> usize {
        if a <= b {
            b - a
        } else {
            2 * self.capacity - a + b
        }
    }
}

impl<T> Drop for RingBuffer<T> {
    /// Drops all non-empty slots.
    fn drop(&mut self) {
        let mut head = self.head.load(Ordering::Relaxed);
        let tail = self.tail.load(Ordering::Relaxed);

        // Loop over all slots that hold a value and drop them.
        while head != tail {
            unsafe {
                self.slot(head).drop_in_place();
            }
            head = self.increment(head, 1);
        }

        // Finally, deallocate the buffer, but don't run any destructors.
        unsafe {
            Vec::from_raw_parts(self.buffer, 0, self.capacity);
        }
    }
}

/// The producer side of a [`RingBuffer`].
///
/// Can be moved between threads,
/// but references from different threads are not allowed
/// (i.e. it is [`Send`] but not [`Sync`]).
///
/// Can only be created with [`RingBuffer::split()`].
///
/// # Examples
///
/// ```
/// use rtrb::RingBuffer;
///
/// let (producer, consumer) = RingBuffer::<f32>::new(1000).split();
/// ```
pub struct Producer<T> {
    /// The inner representation of the queue.
    rb: Arc<RingBuffer<T>>,

    /// A copy of `rb.head` for quick access.
    ///
    /// This value can be stale and sometimes needs to be resynchronized with `rb.head`.
    head: Cell<usize>,

    /// A copy of `rb.tail` for quick access.
    ///
    /// This value is always in sync with `rb.tail`.
    tail: Cell<usize>,

    initialized: usize,
}

unsafe impl<T: Send> Send for Producer<T> {}

impl<T> Producer<T> {
    /// Attempts to push an element into the queue.
    ///
    /// If the queue is full, the element is returned back as an error.
    ///
    /// # Examples
    ///
    /// ```
    /// use rtrb::{RingBuffer, PushError};
    ///
    /// let (mut p, c) = RingBuffer::new(1).split();
    ///
    /// assert_eq!(p.push(10), Ok(()));
    /// assert_eq!(p.push(20), Err(PushError::Full(20)));
    /// ```
    pub fn push(&mut self, value: T) -> Result<(), PushError<T>> {
        if let Some(tail) = self.next_tail() {
            unsafe {
                self.rb.slot(tail).write(value);
            }
            let tail = self.rb.increment1(tail);
            self.rb.tail.store(tail, Ordering::Release);
            self.tail.set(tail);
            Ok(())
        } else {
            Err(PushError::Full(value))
        }
    }

    /// Returns the number of slots available for writing.
    ///
    /// To check for a single available slot,
    /// using [`Producer::is_full()`] is often quicker.
    ///
    /// # Examples
    ///
    /// ```
    /// use rtrb::RingBuffer;
    ///
    /// let (p, c) = RingBuffer::<f32>::new(1024).split();
    ///
    /// assert_eq!(p.slots(), 1024);
    /// ```
    pub fn slots(&self) -> usize {
        let head = self.rb.head.load(Ordering::Acquire);
        self.head.set(head);
        self.rb.capacity - self.rb.distance(head, self.tail.get())
    }

    /// Returns `true` if there are no slots available for writing.
    ///
    /// # Examples
    ///
    /// ```
    /// use rtrb::RingBuffer;
    ///
    /// let (p, c) = RingBuffer::<f32>::new(1).split();
    ///
    /// assert!(!p.is_full());
    /// ```
    pub fn is_full(&self) -> bool {
        self.next_tail().is_none()
    }

    /// Returns the capacity of the queue.
    ///
    /// # Examples
    ///
    /// ```
    /// use rtrb::RingBuffer;
    ///
    /// let (p, c) = RingBuffer::<f32>::new(100).split();
    /// assert_eq!(p.capacity(), 100);
    /// ```
    pub fn capacity(&self) -> usize {
        self.rb.capacity
    }

    /// Get the tail position for writing the next slot, if available.
    ///
    /// This is a strict subset of the functionality implemented in push_slices().
    /// For performance, this special case is immplemented separately.
    fn next_tail(&self) -> Option<usize> {
        let tail = self.tail.get();

        // Check if the queue is *possibly* full.
        if self.rb.distance(self.head.get(), tail) == self.rb.capacity {
            // Refresh the head ...
            let head = self.rb.head.load(Ordering::Acquire);
            self.head.set(head);

            // ... and check if it's *really* full.
            if self.rb.distance(head, tail) == self.rb.capacity {
                return None;
            }
        }
        Some(tail)
    }
}

impl<T> Producer<T>
where
    T: Default,
{
    /// Returns mutable slices for `n` slots and advances the write position when done.
    ///
    /// If not enough slots are available for writing, an error is returned.
    ///
    /// # Examples
    ///
    /// ```
    /// use rtrb::RingBuffer;
    ///
    /// let (mut p, mut c) = RingBuffer::new(3).split();
    ///
    /// assert!(p.push(10).is_ok());
    /// assert_eq!(c.pop(), Ok(10));
    ///
    /// if let Ok(slices) = p.push_slices(3) {
    ///     assert_eq!(slices.first.len(), 2);
    ///     slices.first[0] = 20;
    ///     slices.first[1] = 30;
    ///     assert_eq!(slices.second.len(), 1);
    ///     slices.second[0] = 40;
    /// } else {
    ///     unreachable!();
    /// }
    ///
    /// assert_eq!(c.pop(), Ok(20));
    /// assert_eq!(c.pop(), Ok(30));
    /// assert_eq!(c.pop(), Ok(40));
    /// ```
    pub fn push_slices(&mut self, n: usize) -> Result<PushSlices<'_, T>, SlicesError> {
        let tail = self.tail.get();

        // Check if the queue has *possibly* not enough slots.
        if self.rb.capacity - self.rb.distance(self.head.get(), tail) < n {
            // Refresh the head ...
            let head = self.rb.head.load(Ordering::Acquire);
            self.head.set(head);

            // ... and check if there *really* are not enough slots.
            let slots = self.rb.capacity - self.rb.distance(head, tail);
            if slots < n {
                return Err(SlicesError::TooFewSlots(slots));
            }
        }

        let tail = if tail < self.rb.capacity {
            tail
        } else {
            tail - self.rb.capacity
        };
        let end = self.rb.capacity.min(tail + n);
        for i in self.initialized.max(tail).min(end)..end {
            unsafe { self.rb.buffer.add(i).write(Default::default()) };
        }
        self.initialized = end;

        let first_len = n.min(self.rb.capacity - tail);
        Ok(PushSlices {
            first: unsafe { std::slice::from_raw_parts_mut(self.rb.buffer.add(tail), first_len) },
            second: unsafe { std::slice::from_raw_parts_mut(self.rb.buffer, n - first_len) },
            producer: self,
        })
    }
}

impl<T> fmt::Debug for Producer<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.pad("Producer { .. }")
    }
}

/// The consumer side of a [`RingBuffer`].
///
/// Can be moved between threads,
/// but references from different threads are not allowed
/// (i.e. it is [`Send`] but not [`Sync`]).
///
/// Can only be created with [`RingBuffer::split()`].
///
/// # Examples
///
/// ```
/// use rtrb::RingBuffer;
///
/// let (producer, consumer) = RingBuffer::<f32>::new(1000).split();
/// ```
pub struct Consumer<T> {
    /// The inner representation of the queue.
    rb: Arc<RingBuffer<T>>,

    /// A copy of `rb.head` for quick access.
    ///
    /// This value is always in sync with `rb.head`.
    head: Cell<usize>,

    /// A copy of `rb.tail` for quick access.
    ///
    /// This value can be stale and sometimes needs to be resynchronized with `rb.tail`.
    tail: Cell<usize>,
}

unsafe impl<T: Send> Send for Consumer<T> {}

impl<T> Consumer<T> {
    /// Attempts to pop an element from the queue.
    ///
    /// If the queue is empty, an error is returned.
    ///
    /// To obtain an [`Option<T>`](std::option::Option),
    /// use [`.ok()`](std::result::Result::ok) on the result.
    ///
    /// # Examples
    ///
    /// ```
    /// use rtrb::{PopError, RingBuffer};
    ///
    /// let (mut p, mut c) = RingBuffer::new(1).split();
    ///
    /// assert_eq!(p.push(10), Ok(()));
    /// assert_eq!(c.pop(), Ok(10));
    /// assert_eq!(c.pop(), Err(PopError::Empty));
    ///
    /// assert_eq!(p.push(20), Ok(()));
    /// assert_eq!(c.pop().ok(), Some(20));
    /// ```
    pub fn pop(&mut self) -> Result<T, PopError> {
        if let Some(head) = self.next_head() {
            let value = unsafe { self.rb.slot(head).read() };
            let head = self.rb.increment1(head);
            self.rb.head.store(head, Ordering::Release);
            self.head.set(head);
            Ok(value)
        } else {
            Err(PopError::Empty)
        }
    }

    /// Attempts to read an element from the queue without removing it.
    ///
    /// If the queue is empty, an error is returned.
    ///
    /// # Examples
    ///
    /// ```
    /// use rtrb::{PeekError, RingBuffer};
    ///
    /// let (mut p, c) = RingBuffer::new(1).split();
    ///
    /// assert_eq!(c.peek(), Err(PeekError::Empty));
    /// assert_eq!(p.push(10), Ok(()));
    /// assert_eq!(c.peek(), Ok(&10));
    /// assert_eq!(c.peek(), Ok(&10));
    /// ```
    pub fn peek(&self) -> Result<&T, PeekError> {
        if let Some(head) = self.next_head() {
            Ok(unsafe { &*self.rb.slot(head) })
        } else {
            Err(PeekError::Empty)
        }
    }

    /// Returns slices for `n` slots, drops their contents when done
    /// and advances the read position.
    ///
    /// If not enough slots are available for reading, an error is returned.
    ///
    /// # Examples
    ///
    /// ```
    /// use rtrb::{RingBuffer, SlicesError};
    ///
    /// let (mut p, mut c) = RingBuffer::new(3).split();
    ///
    /// assert_eq!(p.push(10), Ok(()));
    /// assert_eq!(c.pop_slices(2).unwrap_err(), SlicesError::TooFewSlots(1));
    /// assert_eq!(p.push(20), Ok(()));
    ///
    /// if let Ok(slices) = c.pop_slices(2) {
    ///     assert_eq!(slices.first, &[10, 20]);
    ///     assert_eq!(slices.second, &[]);
    ///     // slices implements IntoIterator:
    ///     assert_eq!(slices.into_iter().collect::<Vec<_>>(), [&10, &20]);
    /// } else {
    ///     unreachable!();
    /// }
    ///
    /// assert_eq!(c.pop_slices(2).unwrap_err(), SlicesError::TooFewSlots(0));
    /// assert_eq!(p.push(30), Ok(()));
    /// assert_eq!(p.push(40), Ok(()));
    ///
    /// if let Ok(slices) = c.pop_slices(2) {
    ///     assert_eq!(slices.first, &[30]);
    ///     assert_eq!(slices.second, &[40]);
    /// } else {
    ///     unreachable!();
    /// };
    /// ```
    ///
    /// Items are dropped as soon as [`PopSlices`] goes out of scope:
    ///
    /// ```
    /// use rtrb::RingBuffer;
    ///
    /// static mut DROPS: i32 = 0;
    /// fn dropped(n: i32) -> bool { unsafe { DROPS == n } }
    /// #[derive(Debug)]
    /// struct Thing;
    /// impl Drop for Thing {
    ///     fn drop(&mut self) { unsafe { DROPS += 1; } }
    /// }
    ///
    /// {
    ///     let (mut p, mut c) = RingBuffer::new(2).split();
    ///
    ///     assert!(p.push(Thing).is_ok()); // 1
    ///     assert!(p.push(Thing).is_ok()); // 2
    ///     if let Ok(thing) = c.pop() {
    ///         // "thing" has been *moved* out of the queue but not yet dropped
    ///         assert!(dropped(0));
    ///     } else {
    ///         unreachable!();
    ///     }
    ///     // First Thing has been dropped when "thing" went out of scope:
    ///     assert!(dropped(1));
    ///     assert!(p.push(Thing).is_ok()); // 3
    ///
    ///     if let Ok(slices) = c.pop_slices(2) {
    ///         assert_eq!(slices.first.len(), 1);
    ///         assert_eq!(slices.second.len(), 1);
    ///         // The requested two Things haven't been dropped yet:
    ///         assert!(dropped(1));
    ///     } else {
    ///         unreachable!();
    ///     }
    ///     // Two Things have been dropped when "slices" went out of scope:
    ///     assert!(dropped(3));
    ///     assert!(p.push(Thing).is_ok()); // 4
    /// }
    /// // Last Thing has been dropped when ring buffer went out of scope:
    /// assert!(dropped(4));
    /// ```
    pub fn pop_slices(&mut self, n: usize) -> Result<PopSlices<'_, T>, SlicesError> {
        let (first, second) = self.slices(n)?;
        Ok(PopSlices {
            first,
            second,
            consumer: self,
        })
    }

    /// Returns slices for `n` slots.
    ///
    /// This does *not* advance the read position.
    ///
    /// If not enough slots are available for reading, an error is returned.
    ///
    /// # Examples
    ///
    /// ```
    /// use rtrb::{RingBuffer, SlicesError};
    ///
    /// let (mut p, c) = RingBuffer::new(2).split();
    ///
    /// assert_eq!(p.push(10), Ok(()));
    /// assert_eq!(c.peek_slices(2).unwrap_err(), SlicesError::TooFewSlots(1));
    /// assert_eq!(p.push(20), Ok(()));
    ///
    /// if let Ok(slices) = c.peek_slices(2) {
    ///     assert_eq!(slices.first, &[10, 20]);
    ///     assert_eq!(slices.second, &[]);
    ///
    ///     let mut v = Vec::<i32>::new();
    ///     v.extend(slices); // slices implements IntoIterator!
    ///     assert_eq!(v, [10, 20]);
    /// } else {
    ///     unreachable!();
    /// }
    /// // The two elements have *not* been removed:
    /// assert_eq!(c.slots(), 2);
    /// ```
    pub fn peek_slices(&self, n: usize) -> Result<PeekSlices<'_, T>, SlicesError> {
        let (first, second) = self.slices(n)?;
        Ok(PeekSlices { first, second })
    }

    /// Returns the number of slots available for reading.
    ///
    /// To check for a single available slot,
    /// using [`Consumer::is_empty()`] is often quicker.
    ///
    /// # Examples
    ///
    /// ```
    /// use rtrb::RingBuffer;
    ///
    /// let (p, c) = RingBuffer::<f32>::new(1024).split();
    ///
    /// assert_eq!(c.slots(), 0);
    /// ```
    pub fn slots(&self) -> usize {
        let tail = self.rb.tail.load(Ordering::Acquire);
        self.tail.set(tail);
        self.rb.distance(self.head.get(), tail)
    }

    /// Returns `true` if there are no slots available for reading.
    ///
    /// # Examples
    ///
    /// ```
    /// use rtrb::RingBuffer;
    ///
    /// let (p, c) = RingBuffer::<f32>::new(1).split();
    ///
    /// assert!(c.is_empty());
    /// ```
    pub fn is_empty(&self) -> bool {
        self.next_head().is_none()
    }

    /// Returns the capacity of the queue.
    ///
    /// # Examples
    ///
    /// ```
    /// use rtrb::RingBuffer;
    ///
    /// let (p, c) = RingBuffer::<f32>::new(100).split();
    /// assert_eq!(c.capacity(), 100);
    /// ```
    pub fn capacity(&self) -> usize {
        self.rb.capacity
    }

    /// Get the head position for reading the next slot, if available.
    ///
    /// This is a strict subset of the functionality implemented in pop_slices()/peek_slices().
    /// For performance, this special case is immplemented separately.
    fn next_head(&self) -> Option<usize> {
        let head = self.head.get();

        // Check if the queue is *possibly* empty.
        if head == self.tail.get() {
            // Refresh the tail ...
            let tail = self.rb.tail.load(Ordering::Acquire);
            self.tail.set(tail);

            // ... and check if it's *really* empty.
            if head == tail {
                return None;
            }
        }
        Some(head)
    }

    fn advance_head(&self, head: usize, n: usize) {
        let head = self.rb.increment(head, n);
        self.rb.head.store(head, Ordering::Release);
        self.head.set(head);
    }

    /// Get slices holding `n` slots.
    fn slices(&self, n: usize) -> Result<(&[T], &[T]), SlicesError> {
        let head = self.head.get();

        // Check if the queue has *possibly* not enough slots.
        if self.rb.distance(head, self.tail.get()) < n {
            // Refresh the tail ...
            let tail = self.rb.tail.load(Ordering::Acquire);
            self.tail.set(tail);

            // ... and check if there *really* are not enough slots.
            let slots = self.rb.distance(head, tail);
            if slots < n {
                return Err(SlicesError::TooFewSlots(slots));
            }
        }

        let head = if head < self.rb.capacity {
            head
        } else {
            head - self.rb.capacity
        };
        let first_len = n.min(self.rb.capacity - head);
        Ok((
            unsafe { std::slice::from_raw_parts(self.rb.slot(head), first_len) },
            unsafe { std::slice::from_raw_parts(self.rb.buffer, n - first_len) },
        ))
    }
}

/// Contains two mutable slices from the ring buffer.
/// When this structure is dropped (falls out of scope), the slots are made available for reading.
///
/// This is returned from [`Producer::push_slices()`].
#[derive(Debug)]
pub struct PushSlices<'a, T> {
    /// First part of the requested slots.
    ///
    /// Can only be empty if `0` slots have been requested.
    pub first: &'a mut [T],
    /// Second part of the requested slots.
    ///
    /// If `first` contains all requested slots, this is empty.
    pub second: &'a mut [T],
    producer: &'a Producer<T>,
}

/// Contains two slices from the ring buffer.
///
/// This is returned from [`Consumer::peek_slices()`].
///
/// It implements [`IntoIterator`] by chaining the two slices together,
/// and it can therefore, for example, be iterated with a `for` loop.
#[derive(Debug)]
pub struct PeekSlices<'a, T> {
    /// First part of the requested slots.
    ///
    /// Can only be empty if `0` slots have been requested.
    pub first: &'a [T],
    /// Second part of the requested slots.
    ///
    /// If `first` contains all requested slots, this is empty.
    pub second: &'a [T],
}

/// Contains two slices from the ring buffer. When this structure is dropped (falls out of scope),
/// the contents of the slices will be dropped and the read position will be advanced.
///
/// This is returned from [`Consumer::pop_slices()`].
///
/// It implements [`IntoIterator`] by chaining the two slices together,
/// and it can therefore, for example, be iterated with a `for` loop.
#[derive(Debug)]
pub struct PopSlices<'a, T> {
    /// First part of the requested slots.
    ///
    /// Can only be empty if `0` slots have been requested.
    pub first: &'a [T],
    /// Second part of the requested slots.
    ///
    /// If `first` contains all requested slots, this is empty.
    pub second: &'a [T],
    consumer: &'a Consumer<T>,
}

impl<'a, T> Drop for PushSlices<'a, T> {
    /// Makes the requested slots available for reading.
    fn drop(&mut self) {
        let tail = self.producer.rb.increment(
            self.producer.tail.get(),
            self.first.len() + self.second.len(),
        );
        self.producer.rb.tail.store(tail, Ordering::Release);
        self.producer.tail.set(tail);
    }
}

impl<'a, T> Drop for PopSlices<'a, T> {
    /// Drops all requested slots and advances the read position,
    /// making the space available for writing again.
    fn drop(&mut self) {
        // Safety: the exclusive reference taken by pop_slices()
        //         makes sure nobody else has access to the buffer.
        let head = self.consumer.head.get();
        // Safety: head has not yet been incremented
        let ptr = unsafe { self.consumer.rb.slot(head) };
        for i in 0..self.first.len() {
            unsafe {
                ptr.add(i).drop_in_place();
            }
        }
        let ptr = self.consumer.rb.buffer;
        for i in 0..self.second.len() {
            unsafe {
                ptr.add(i).drop_in_place();
            }
        }
        self.consumer
            .advance_head(head, self.first.len() + self.second.len());
    }
}

impl<'a, T> IntoIterator for PeekSlices<'a, T> {
    type Item = &'a T;
    type IntoIter = std::iter::Chain<std::slice::Iter<'a, T>, std::slice::Iter<'a, T>>;
    fn into_iter(self) -> Self::IntoIter {
        self.first.iter().chain(self.second)
    }
}

impl<'a, T> IntoIterator for PopSlices<'a, T> {
    type Item = &'a T;
    type IntoIter = std::iter::Chain<std::slice::Iter<'a, T>, std::slice::Iter<'a, T>>;
    fn into_iter(self) -> Self::IntoIter {
        self.first.iter().chain(self.second)
    }
}

impl<T> fmt::Debug for Consumer<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.pad("Consumer { .. }")
    }
}
