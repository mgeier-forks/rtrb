use core::cell::Cell;

use super::arc_ring_buffer::ArcRingBuffer;

use super::{
    chunks::{WriteChunk, WriteChunkUninit},
    ChunkError, CopyToUninit as _, PushError, RingBuffer,
};

// Only used in documentation:
#[allow(unused_imports)]
use super::Consumer;

/// The producer side of a [`RingBuffer`].
///
/// Can be moved between threads,
/// but references from different threads are not allowed
/// (i.e. it is [`Send`] but not [`Sync`]).
///
/// Can only be created with [`RingBuffer::new()`]
/// (together with its counterpart, the [`Consumer`]).
///
/// Individual elements can be moved into the ring buffer with [`Producer::push()`],
/// multiple elements at once can be written with [`Producer::write_chunk()`],
/// [`Producer::write_chunk_uninit()`] and [`Producer::push_partial_slice()`].
///
/// The number of free slots currently available for writing can be obtained with
/// [`Producer::slots()`].
///
/// When the `Producer` is dropped, [`Consumer::is_abandoned()`] will return `true`.
/// This can be used as a crude way to communicate to the receiving thread
/// that no more data will be produced.
/// When the `Producer` is dropped after the [`Consumer`] has already been dropped,
/// [`RingBuffer::drop()`] will be called, freeing the allocated memory.
#[derive(Debug, PartialEq, Eq)]
pub struct Producer<T> {
    /// A reference to the ring buffer.
    buffer: ArcRingBuffer<T>,

    /// A copy of `buffer.head` for quick access.
    ///
    /// This value can be stale and sometimes needs to be resynchronized with `buffer.head`.
    cached_head: Cell<usize>,

    /// A copy of `buffer.tail` for quick access.
    ///
    /// This value is always in sync with `buffer.tail`.
    // NB: Caching the tail seems to have little effect on Intel CPUs, but it seems to
    //     improve performance on AMD CPUs, see https://github.com/mgeier/rtrb/pull/132
    cached_tail: Cell<usize>,
}

// SAFETY: After moving a Producer to another thread, there is still only a single thread
// that can access the producer side of the queue.
unsafe impl<T: Send> Send for Producer<T> {}

impl<T> Producer<T> {
    pub(super) unsafe fn new(buffer: ArcRingBuffer<T>, head: usize, tail: usize) -> Self {
        Self {
            buffer,
            cached_head: Cell::new(head),
            cached_tail: Cell::new(tail),
        }
    }

    /// Attempts to push an element into the queue.
    ///
    /// The element is *moved* into the ring buffer and its slot
    /// is made available to be read by the [`Consumer`].
    ///
    /// # Errors
    ///
    /// If the queue is full, the element is returned back as an error.
    ///
    /// # Examples
    ///
    /// ```
    /// use rtrb::{RingBuffer, PushError};
    ///
    /// let (mut p, c) = RingBuffer::new(1);
    ///
    /// assert_eq!(p.push(10), Ok(()));
    /// assert_eq!(p.push(20), Err(PushError::Full(20)));
    /// ```
    pub fn push(&mut self, value: T) -> Result<(), PushError<T>> {
        if let Some(tail) = self.next_tail() {
            // SAFETY: tail points to an empty slot.
            unsafe {
                self.buffer.slot_ptr(tail).write(value);
            }
            let tail = self.buffer.increment1(tail);
            // SAFETY: The new `tail` has been calculated correctly.
            unsafe {
                self.buffer.set_tail(tail);
            }
            self.cached_tail.set(tail);
            Ok(())
        } else {
            Err(PushError::Full(value))
        }
    }

    /// Returns the number of slots available for writing.
    ///
    /// Since items can be concurrently consumed on another thread, the actual number
    /// of available slots may increase at any time (up to the [`RingBuffer::capacity()`]).
    ///
    /// To check for a single available slot,
    /// using [`Producer::is_full()`] is often quicker
    /// (because it might not have to check an atomic variable).
    ///
    /// # Examples
    ///
    /// ```
    /// use rtrb::RingBuffer;
    ///
    /// let (p, c) = RingBuffer::<f32>::new(1024);
    ///
    /// assert_eq!(p.slots(), 1024);
    /// ```
    pub fn slots(&self) -> usize {
        let head = self.buffer.head();
        self.cached_head.set(head);
        self.buffer.capacity() - self.buffer.distance(head, self.cached_tail.get())
    }

    /// Returns the number of cached slots.
    ///
    /// In many cases, this will not provide all available slots,
    /// but it might be marginally faster than [`Producer::slots()`]
    /// because it doesn't access the atomic read index.
    pub fn cached_slots(&self) -> usize {
        let head = self.cached_head.get();
        let tail = self.cached_tail.get();
        self.buffer.capacity() - self.buffer.distance(head, tail)
    }

    /// Returns `true` if there are currently no slots available for writing.
    ///
    /// A full ring buffer might cease to be full at any time
    /// if the corresponding [`Consumer`] is consuming items in another thread.
    ///
    /// # Examples
    ///
    /// ```
    /// use rtrb::RingBuffer;
    ///
    /// let (p, c) = RingBuffer::<f32>::new(1);
    ///
    /// assert!(!p.is_full());
    /// ```
    ///
    /// Since items can be concurrently consumed on another thread, the ring buffer
    /// might not be full for long:
    ///
    /// ```
    /// # use rtrb::RingBuffer;
    /// # let (p, c) = RingBuffer::<f32>::new(1);
    /// if p.is_full() {
    ///     // The buffer might be full, but it might as well not be
    ///     // if an item was just consumed on another thread.
    /// }
    /// ```
    ///
    /// However, if it's not full, another thread cannot change that:
    ///
    /// ```
    /// # use rtrb::RingBuffer;
    /// # let (p, c) = RingBuffer::<f32>::new(1);
    /// if !p.is_full() {
    ///     // At least one slot is guaranteed to be available for writing.
    /// }
    /// ```
    pub fn is_full(&self) -> bool {
        self.next_tail().is_none()
    }

    /// Returns `true` if the corresponding [`Consumer`] has been destroyed.
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
    /// let (mut p, c) = RingBuffer::new(7);
    /// assert!(!p.is_abandoned());
    /// assert_eq!(p.push(10), Ok(()));
    /// drop(c);
    /// // The items that are still in the ring buffer are not accessible anymore.
    /// assert!(p.is_abandoned());
    /// // Even though it's futile, items can still be written:
    /// assert_eq!(p.push(11), Ok(()));
    /// ```
    ///
    /// Since the consumer can be concurrently dropped on another thread,
    /// the producer might become abandoned at any time:
    ///
    /// ```
    /// # use rtrb::RingBuffer;
    /// # let (p, c) = RingBuffer::<i32>::new(1);
    /// if !p.is_abandoned() {
    ///     // Right now, the consumer might still be alive, but it might as well not be
    ///     // if another thread has just dropped it.
    /// }
    /// ```
    ///
    /// However, if it already is abandoned, it will stay that way:
    ///
    /// ```
    /// # use rtrb::RingBuffer;
    /// # let (p, c) = RingBuffer::<i32>::new(1);
    /// if p.is_abandoned() {
    ///     // The consumer does definitely not exist anymore.
    /// }
    /// ```
    pub fn is_abandoned(&self) -> bool {
        self.buffer.is_abandoned()
    }

    /// Returns a read-only reference to the ring buffer.
    pub fn buffer(&self) -> &RingBuffer<T> {
        &self.buffer
    }

    /// Get the tail position for writing the next slot, if available.
    ///
    /// This is a strict subset of the functionality implemented in `write_chunk_uninit()`.
    /// For performance, this special case is immplemented separately.
    fn next_tail(&self) -> Option<usize> {
        let tail = self.cached_tail.get();

        // Check if the queue is *possibly* full.
        if self.buffer.distance(self.cached_head.get(), tail) == self.buffer.capacity() {
            // Refresh the head ...
            let head = self.buffer.head();
            self.cached_head.set(head);

            // ... and check if it's *really* full.
            if self.buffer.distance(head, tail) == self.buffer.capacity() {
                return None;
            }
        }
        Some(tail)
    }

    /// Returns `n` slots (initially containing their [`Default`] value) for writing.
    ///
    /// [`WriteChunk::as_mut_slices()`] provides mutable access to the slots.
    /// After writing to those slots, they explicitly have to be made available
    /// to be read by the [`Consumer`] by calling [`WriteChunk::commit()`]
    /// or [`WriteChunk::commit_all()`].
    ///
    /// For an alternative that does not require the trait bound [`Default`],
    /// see [`Producer::write_chunk_uninit()`].
    ///
    /// If items are supposed to be moved from an iterator into the ring buffer,
    /// [`Producer::write_chunk_uninit()`] followed by [`WriteChunkUninit::fill_from_iter()`]
    /// can be used.
    ///
    /// # Errors
    ///
    /// If not enough slots are available, an error
    /// (containing the number of available slots) is returned.
    /// Use [`Producer::slots()`] to obtain the number of available slots beforehand.
    ///
    /// # Examples
    ///
    /// See the documentation of the [`chunks`](crate::chunks#examples) module.
    pub fn write_chunk(&mut self, n: usize) -> Result<WriteChunk<'_, T>, ChunkError>
    where
        T: Default,
    {
        self.write_chunk_uninit(n).map(WriteChunk::from)
    }

    /// Returns `n` (uninitialized) slots for writing.
    ///
    /// [`WriteChunkUninit::as_mut_slices()`] provides mutable access
    /// to the uninitialized slots.
    /// After writing to those slots, they explicitly have to be made available
    /// to be read by the [`Consumer`] by calling [`WriteChunkUninit::commit()`]
    /// or [`WriteChunkUninit::commit_all()`].
    ///
    /// Alternatively, [`WriteChunkUninit::fill_from_iter()`] can be used
    /// to move items from an iterator into the available slots.
    /// All moved items are automatically made available to be read by the [`Consumer`].
    ///
    /// # Errors
    ///
    /// If not enough slots are available, an error
    /// (containing the number of available slots) is returned.
    /// Use [`Producer::slots()`] to obtain the number of available slots beforehand.
    ///
    /// # Safety
    ///
    /// This function itself is safe, as is [`WriteChunkUninit::fill_from_iter()`].
    /// However, when using [`WriteChunkUninit::as_mut_slices()`],
    /// the user has to make sure that the relevant slots have been initialized
    /// before calling [`WriteChunkUninit::commit()`] or [`WriteChunkUninit::commit_all()`].
    ///
    /// For a safe alternative that provides mutable slices of [`Default`]-initialized slots,
    /// see [`Producer::write_chunk()`].
    ///
    /// # Examples
    ///
    /// See the documentation of the [`chunks`](crate::chunks#examples) module.
    pub fn write_chunk_uninit(&mut self, n: usize) -> Result<WriteChunkUninit<'_, T>, ChunkError> {
        let tail = self.cached_tail.get();
        let b = self.buffer();

        // Check if the queue has *possibly* not enough slots.
        if b.capacity() - b.distance(self.cached_head.get(), tail) < n {
            // Refresh the head ...
            let head = b.head();
            self.cached_head.set(head);

            // ... and check if there *really* are not enough slots.
            let slots = b.capacity() - b.distance(head, tail);
            if slots < n {
                return Err(ChunkError::TooFewSlots(slots));
            }
        }
        let offset = b.collapse_position(tail);
        // SAFETY: `offset` has been set to a valid position.
        Ok(unsafe { WriteChunkUninit::new(self, n, offset) })
    }

    pub(super) unsafe fn advance_unchecked(&self, n: usize) {
        let tail = self.buffer.increment(self.cached_tail.get(), n);
        // SAFETY: The user must make sure that `n` slots have been written.
        unsafe {
            self.buffer.set_tail(tail);
        }
        self.cached_tail.set(tail);
    }
}

impl<T: Copy> Producer<T> {
    /// Copies as many items as possible from the given `slice` into the ring buffer.
    ///
    /// The written slots are automatically made available to be read by the [`Consumer`].
    ///
    /// Returns two sub-slices of `slice`:
    /// - The part that has been copied into the ring buffer (possibly empty).
    /// - The unused remainder (possibly empty).
    ///
    /// To copy an entire slice (and fail otherwise), [`Producer::push_entire_slice()`] can be used.
    ///
    /// # Examples
    ///
    /// ```
    /// use rtrb::Producer;
    ///
    /// fn push_at_least_one_element<'a>(
    ///     p: &mut Producer<i32>,
    ///     s: &'a [i32],
    /// ) -> Result<&'a [i32], &'a [i32]> {
    ///     match p.push_partial_slice(s) {
    ///         ([], remainder) => Err(remainder),
    ///         (_, remainder) => Ok(remainder),
    ///     }
    /// }
    ///
    /// fn block_while_pushing_entire_slice(p: &mut Producer<i32>, mut s: &[i32]) {
    ///     while let (_, remainder @ [_, ..]) = p.push_partial_slice(s) {
    ///         std::thread::yield_now();
    ///         s = remainder;
    ///     }
    /// }
    /// ```
    ///
    /// For more examples, see the documentation of the [`chunks`](crate::chunks#examples) module.
    #[must_use]
    pub fn push_partial_slice<'a>(&mut self, slice: &'a [T]) -> (&'a [T], &'a [T]) {
        let slots = if self.cached_slots() < slice.len() {
            slice.len().min(self.slots())
        } else {
            slice.len()
        };
        let (pushed, remainder) = slice.split_at(slots);
        // With MSRV 1.58, unwrap_unchecked() can be used.
        match self.push_entire_slice(pushed) {
            Ok(()) => {}
            // SAFETY: The requested slots are available.
            Err(_) => unsafe { core::hint::unreachable_unchecked() },
        };
        (pushed, remainder)
    }

    /// Copies all items from the given `slice` into the ring buffer.
    ///
    /// The written slots are automatically made available to be read by the [`Consumer`].
    ///
    /// To copy only into the available slots, [`Producer::push_partial_slice()`] can be used.
    ///
    /// # Errors
    ///
    /// If not enough free space is available in the ring buffer,
    /// a [`ChunkError`] with the available slots is returned.
    pub fn push_entire_slice(&mut self, slice: &[T]) -> Result<(), ChunkError> {
        let mut chunk = self.write_chunk_uninit(slice.len())?;
        let (one, two) = chunk.as_mut_slices();
        let mid = one.len();
        // NB: If slice.is_empty(), chunk will be empty as well and the following are no-ops:
        slice[..mid].copy_to_uninit(one);
        slice[mid..].copy_to_uninit(two);
        // SAFETY: All slots have been initialized
        unsafe { chunk.commit_all() };
        Ok(())
    }
}
