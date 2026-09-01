use core::cell::Cell;
use core::sync::atomic::Ordering;

use super::arc_ring_buffer::ArcRingBuffer;

use super::{PeekError, PopError, RingBuffer};

/// The consumer side of a [`RingBuffer`].
///
/// Can be moved between threads,
/// but references from different threads are not allowed
/// (i.e. it is [`Send`] but not [`Sync`]).
///
/// Can only be created with [`RingBuffer::new()`]
/// (together with its counterpart, the [`Producer`]).
///
/// Individual elements can be moved out of the ring buffer with [`Consumer::pop()`],
/// multiple elements at once can be read with [`Consumer::read_chunk()`],
/// [`Consumer::pop_partial_slice()`] and [`Consumer::pop_partial_slice_uninit()`].
///
/// The number of slots currently available for reading can be obtained with
/// [`Consumer::slots()`].
///
/// When the `Consumer` is dropped, [`Producer::is_abandoned()`] will return `true`.
/// This can be used as a crude way to communicate to the sending thread
/// that no more data will be consumed.
/// When the `Consumer` is dropped after the [`Producer`] has already been dropped,
/// [`RingBuffer::drop()`] will be called, freeing the allocated memory.
#[derive(Debug, PartialEq, Eq)]
pub struct Consumer<T> {
    /// A reference to the ring buffer.
    buffer: ArcRingBuffer<T>,

    /// A copy of `buffer.head` for quick access.
    ///
    /// This value is always in sync with `buffer.head`.
    // NB: Caching the head seems to have little effect on Intel CPUs, but it seems to
    //     improve performance on AMD CPUs, see https://github.com/mgeier/rtrb/pull/132
    cached_head: Cell<usize>,

    /// A copy of `buffer.tail` for quick access.
    ///
    /// This value can be stale and sometimes needs to be resynchronized with `buffer.tail`.
    cached_tail: Cell<usize>,
}

// SAFETY: After moving a Consumer to another thread, there is still only a single thread
// that can access the consumer side of the queue.
unsafe impl<T: Send> Send for Consumer<T> {}

impl<T> Consumer<T> {
    /// Attempts to pop an element from the queue.
    ///
    /// The element is *moved* out of the ring buffer and its slot
    /// is made available to be filled by the [`Producer`] again.
    ///
    /// # Errors
    ///
    /// If the queue is empty, an error is returned.
    ///
    /// # Examples
    ///
    /// ```
    /// use rtrb::{PopError, RingBuffer};
    ///
    /// let (mut p, mut c) = RingBuffer::new(1);
    ///
    /// assert_eq!(p.push(10), Ok(()));
    /// assert_eq!(c.pop(), Ok(10));
    /// assert_eq!(c.pop(), Err(PopError::Empty));
    /// ```
    ///
    /// To obtain an [`Option<T>`](Option), use [`.ok()`](Result::ok) on the result.
    ///
    /// ```
    /// # use rtrb::RingBuffer;
    /// # let (mut p, mut c) = RingBuffer::new(1);
    /// assert_eq!(p.push(20), Ok(()));
    /// assert_eq!(c.pop().ok(), Some(20));
    /// ```
    pub fn pop(&mut self) -> Result<T, PopError> {
        if let Some(head) = self.next_head() {
            // SAFETY: head points to an initialized slot.
            let value = unsafe { self.buffer.slot_ptr(head).read() };
            let head = self.buffer.increment1(head);
            self.buffer.head.store(head, Ordering::Release);
            self.cached_head.set(head);
            Ok(value)
        } else {
            Err(PopError::Empty)
        }
    }

    /// Attempts to read an element from the queue without removing it.
    ///
    /// # Errors
    ///
    /// If the queue is empty, an error is returned.
    ///
    /// # Examples
    ///
    /// ```
    /// use rtrb::{PeekError, RingBuffer};
    ///
    /// let (mut p, c) = RingBuffer::new(1);
    ///
    /// assert_eq!(c.peek(), Err(PeekError::Empty));
    /// assert_eq!(p.push(10), Ok(()));
    /// assert_eq!(c.peek(), Ok(&10));
    /// assert_eq!(c.peek(), Ok(&10));
    /// ```
    ///
    /// Note that `peek()` takes a shared reference to `self`,
    /// which means that other methods that take `&self` can be called
    /// while the returned reference is still in use.
    /// However, calling methods that take `&mut self`
    /// (like [`Consumer::pop()`] and [`Consumer::read_chunk()`]) leads to a compiler error:
    ///
    /// ```compile_fail
    /// use rtrb::RingBuffer;
    ///
    /// let (mut p, mut c) = RingBuffer::new(8);
    /// p.push(10).unwrap();
    /// let shared_ref = c.peek().unwrap();
    /// let value = c.pop().unwrap();
    /// assert_eq!(shared_ref, &10);
    /// ```
    pub fn peek(&self) -> Result<&T, PeekError> {
        if let Some(head) = self.next_head() {
            // SAFETY: head points to an initialized slot.
            Ok(unsafe { &*self.buffer.slot_ptr(head) })
        } else {
            Err(PeekError::Empty)
        }
    }

    /// Returns the number of slots available for reading.
    ///
    /// Since items can be concurrently produced on another thread, the actual number
    /// of available slots may increase at any time (up to the [`RingBuffer::capacity()`]).
    ///
    /// To check for a single available slot,
    /// using [`Consumer::is_empty()`] is often quicker
    /// (because it might not have to check an atomic variable).
    ///
    /// # Examples
    ///
    /// ```
    /// use rtrb::RingBuffer;
    ///
    /// let (p, c) = RingBuffer::<f32>::new(1024);
    ///
    /// assert_eq!(c.slots(), 0);
    /// ```
    pub fn slots(&self) -> usize {
        let tail = self.buffer.tail.load(Ordering::Acquire);
        self.cached_tail.set(tail);
        self.buffer.distance(self.cached_head.get(), tail)
    }

    /// Returns the number of cached slots.
    ///
    /// In many cases, this will not provide all available slots,
    /// but it might be marginally faster than [`Consumer::slots()`]
    /// because it doesn't access the atomic write index.
    pub fn cached_slots(&self) -> usize {
        let head = self.cached_head.get();
        let tail = self.cached_tail.get();
        self.buffer.distance(head, tail)
    }

    /// Returns `true` if there are currently no slots available for reading.
    ///
    /// An empty ring buffer might cease to be empty at any time
    /// if the corresponding [`Producer`] is producing items in another thread.
    ///
    /// # Examples
    ///
    /// ```
    /// use rtrb::RingBuffer;
    ///
    /// let (p, c) = RingBuffer::<f32>::new(1);
    ///
    /// assert!(c.is_empty());
    /// ```
    ///
    /// Since items can be concurrently produced on another thread, the ring buffer
    /// might not be empty for long:
    ///
    /// ```
    /// # use rtrb::RingBuffer;
    /// # let (p, c) = RingBuffer::<f32>::new(1);
    /// if c.is_empty() {
    ///     // The buffer might be empty, but it might as well not be
    ///     // if an item was just produced on another thread.
    /// }
    /// ```
    ///
    /// However, if it's not empty, another thread cannot change that:
    ///
    /// ```
    /// # use rtrb::RingBuffer;
    /// # let (p, c) = RingBuffer::<f32>::new(1);
    /// if !c.is_empty() {
    ///     // At least one slot is guaranteed to be available for reading.
    /// }
    /// ```
    pub fn is_empty(&self) -> bool {
        self.next_head().is_none()
    }

    /// Returns `true` if the corresponding [`Producer`] has been destroyed.
    ///
    /// Note that since Rust version 1.74.0 and before `rtrb` version 0.4,
    /// this was not synchronizing with the consumer thread anymore,
    /// see [issue #114](https://github.com/mgeier/rtrb/issues/114).
    /// In `rtrb` version 0.4, the synchronizing behavior has been restored.
    ///
    /// # Examples
    ///
    /// ```
    /// use rtrb::RingBuffer;
    ///
    /// let (mut p, mut c) = RingBuffer::new(7);
    /// assert!(!c.is_abandoned());
    /// assert_eq!(p.push(10), Ok(()));
    /// drop(p);
    /// assert!(c.is_abandoned());
    /// // The items that are left in the ring buffer can still be consumed:
    /// assert_eq!(c.pop(), Ok(10));
    /// ```
    ///
    /// Since the producer can be concurrently dropped on another thread,
    /// the consumer might become abandoned at any time:
    ///
    /// ```
    /// # use rtrb::RingBuffer;
    /// # let (p, c) = RingBuffer::<i32>::new(1);
    /// if !c.is_abandoned() {
    ///     // Right now, the producer might still be alive, but it might as well not be
    ///     // if another thread has just dropped it.
    /// }
    /// ```
    ///
    /// However, if it already is abandoned, it will stay that way:
    ///
    /// ```
    /// # use rtrb::RingBuffer;
    /// # let (p, c) = RingBuffer::<i32>::new(1);
    /// if c.is_abandoned() {
    ///     // The producer does definitely not exist anymore.
    /// }
    /// ```
    pub fn is_abandoned(&self) -> bool {
        self.buffer.is_abandoned()
    }

    /// Returns a read-only reference to the ring buffer.
    pub fn buffer(&self) -> &RingBuffer<T> {
        &self.buffer
    }

    /// Get the head position for reading the next slot, if available.
    ///
    /// This is a strict subset of the functionality implemented in `read_chunk()`.
    /// For performance, this special case is immplemented separately.
    fn next_head(&self) -> Option<usize> {
        let head = self.cached_head.get();

        // Check if the queue is *possibly* empty.
        if head == self.cached_tail.get() {
            // Refresh the tail ...
            let tail = self.buffer.tail.load(Ordering::Acquire);
            self.cached_tail.set(tail);

            // ... and check if it's *really* empty.
            if head == tail {
                return None;
            }
        }
        Some(head)
    }
}
