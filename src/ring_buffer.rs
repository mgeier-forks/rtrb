use alloc::boxed::Box;
use alloc::vec::Vec;
use core::marker::PhantomData;
use core::mem::ManuallyDrop;
use core::sync::atomic::{AtomicU8, AtomicUsize, Ordering};

use crate::cache_padded::CachePadded;

use super::arc_ring_buffer::ArcRingBuffer;
use super::{Consumer, Producer};

const IS_ABANDONED: u8 = 0b10000000;

/// A bounded single-producer single-consumer (SPSC) queue.
///
/// Elements can be written with a [`Producer`] and read with a [`Consumer`],
/// both of which can be obtained with [`RingBuffer::new()`].
///
/// *See also the [crate-level documentation](crate).*
#[derive(Debug)]
pub struct RingBuffer<T> {
    /// The head of the queue.
    ///
    /// This integer is in range `0 .. 2 * capacity`.
    head: CachePadded<AtomicUsize>,

    /// The tail of the queue.
    ///
    /// This integer is in range `0 .. 2 * capacity`.
    tail: CachePadded<AtomicUsize>,

    flags: AtomicU8,

    /// The buffer holding slots.
    data_ptr: *mut T,

    /// The queue capacity.
    capacity: usize,

    /// Indicates that dropping a `RingBuffer<T>` may drop elements of type `T`.
    _marker: PhantomData<T>,
}

impl<T> RingBuffer<T> {
    /// Creates a `RingBuffer` with the given `capacity` and returns [`Producer`] and [`Consumer`].
    ///
    /// # Panics
    ///
    /// Panics if `capacity * size_of::<T>()` exceeds `isize::MAX` bytes or,
    /// when `T` is a zero-sized type, if `capacity` is larger than `usize::MAX / 2`.
    ///
    /// # Examples
    ///
    /// ```
    /// use rtrb::RingBuffer;
    ///
    /// let (producer, consumer) = RingBuffer::<f32>::new(100);
    /// ```
    ///
    /// Specifying an explicit type with the [turbofish](https://turbo.fish/)
    /// is is only necessary if it cannot be deduced by the compiler.
    ///
    /// ```
    /// use rtrb::RingBuffer;
    ///
    /// let (mut producer, consumer) = RingBuffer::new(100);
    /// assert_eq!(producer.push(0.0f32), Ok(()));
    /// ```
    #[allow(clippy::new_ret_no_self)]
    #[must_use]
    pub fn new(capacity: usize) -> (Producer<T>, Consumer<T>) {
        assert!(
            capacity.checked_mul(2).is_some(),
            "capacity exceeds usize::MAX / 2"
        );
        ArcRingBuffer::new(Box::new(Self {
            head: CachePadded::new(AtomicUsize::new(0)),
            tail: CachePadded::new(AtomicUsize::new(0)),
            flags: AtomicU8::new(0),
            data_ptr: ManuallyDrop::new(Vec::with_capacity(capacity)).as_mut_ptr(),
            capacity,
            _marker: PhantomData,
        }))
    }

    /// Returns the capacity of the queue.
    ///
    /// # Examples
    ///
    /// ```
    /// use rtrb::RingBuffer;
    ///
    /// let (producer, consumer) = RingBuffer::<f32>::new(100);
    /// assert_eq!(producer.buffer().capacity(), 100);
    /// assert_eq!(consumer.buffer().capacity(), 100);
    /// // Both producer and consumer of course refer to the same ring buffer:
    /// assert_eq!(producer.buffer(), consumer.buffer());
    /// ```
    pub fn capacity(&self) -> usize {
        self.capacity
    }

    /// Wraps a position from the range `0 .. 2 * capacity` to `0 .. capacity`.
    fn collapse_position(&self, pos: usize) -> usize {
        debug_assert!(pos == 0 || pos < 2 * self.capacity);
        if pos < self.capacity {
            pos
        } else {
            pos - self.capacity
        }
    }

    /// Returns a pointer to the slot at position `pos`.
    ///
    /// If `pos == 0 && capacity == 0`, the returned pointer must not be dereferenced!
    unsafe fn slot_ptr(&self, pos: usize) -> *mut T {
        debug_assert!(pos == 0 || pos < 2 * self.capacity);
        let pos = self.collapse_position(pos);
        // SAFETY: The caller must ensure a valid pos.
        unsafe { self.data_ptr.add(pos) }
    }

    /// Increments a position by going `n` slots forward.
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

    /// Increments a position by going one slot forward.
    ///
    /// This is more efficient than self.increment(..., 1).
    fn increment1(&self, pos: usize) -> usize {
        debug_assert_ne!(self.capacity, 0);
        debug_assert!(pos < 2 * self.capacity);
        if pos < 2 * self.capacity - 1 {
            pos + 1
        } else {
            0
        }
    }

    /// Returns the distance between two positions.
    fn distance(&self, a: usize, b: usize) -> usize {
        debug_assert!(a == 0 || a < 2 * self.capacity);
        debug_assert!(b == 0 || b < 2 * self.capacity);
        if a <= b {
            b - a
        } else {
            2 * self.capacity - a + b
        }
    }

    pub(super) fn is_abandoned(&self) -> bool {
        self.flags.load(Ordering::Acquire) & IS_ABANDONED != 0
    }

    pub(super) fn abandon(&self) -> bool {
        // The "store" part of `fetch_or()` has to use `Release` to make sure that any previous writes
        // to the ring buffer happen before it (in the thread that drops first).
        // The "load" part can be `Relaxed` for the first thread,
        // but it must be `Acquire` for the second one (see below).
        if self.flags.fetch_or(IS_ABANDONED, Ordering::Release) & IS_ABANDONED == 0 {
            // The flag wasn't set before, so we are the first to drop our
            // producer/consumer and it should not be dropped yet.
            false
        } else {
            // The flag was already set, i.e. the other thread has already dropped its
            // consumer/producer and it can be dropped now.

            // However, since the load of `flags` was `Relaxed`,
            // we have to use `Acquire` here to make sure that reading `head` and `tail`
            // in the destructor happens after this point.

            // Ideally, we would use a memory fence like this:
            //core::sync::atomic::fence(Ordering::Acquire);
            // ... but as long as ThreadSanitizer doesn't support fences,
            // we use load(Acquire) as a work-around to avoid false positives:
            let _ = self.flags.load(Ordering::Acquire);
            true
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
            // SAFETY: All slots between head and tail have been initialized.
            unsafe { self.slot_ptr(head).drop_in_place() };
            head = self.increment1(head);
        }

        // Finally, deallocate the buffer, but don't run any destructors.
        // SAFETY: data_ptr and capacity are still valid from the original initialization.
        unsafe { Vec::from_raw_parts(self.data_ptr, 0, self.capacity) };
    }
}

impl<T> PartialEq for RingBuffer<T> {
    /// This method tests for `self` and `other` values to be equal, and is used by `==`.
    ///
    /// # Examples
    ///
    /// ```
    /// use rtrb::RingBuffer;
    ///
    /// let (p1, c1) = RingBuffer::<f32>::new(1000);
    /// assert_eq!(p1.buffer(), c1.buffer());
    ///
    /// let (p2, c2) = RingBuffer::<f32>::new(1000);
    /// assert_ne!(p1.buffer(), p2.buffer());
    /// ```
    fn eq(&self, other: &Self) -> bool {
        core::ptr::eq(self, other)
    }
}

impl<T> Eq for RingBuffer<T> {}
