//! A realtime-safe single-producer single-consumer (SPSC) ring buffer.
//!
//! A [`RingBuffer`] consists of two parts:
//! a [`Producer`] for writing into the ring buffer and
//! a [`Consumer`] for reading from the ring buffer.
//!
//! A fixed-capacity buffer is allocated on construction.
//! After that, no more memory is allocated (unless the type `T` does that internally).
//! Reading from and writing into the ring buffer is *lock-free* and *wait-free*.
//! All reading and writing functions return immediately.
//! Attempts to write to a full buffer return an error;
//! values inside the buffer are *not* overwritten.
//! Attempts to read from an empty buffer return an error as well.
//! Only a single thread can write into the ring buffer and a single thread
//! (typically a different one) can read from the ring buffer.
//! If the queue is empty, there is no way for the reading thread to wait
//! for new data, other than trying repeatedly until reading succeeds.
//! Similarly, if the queue is full, there is no way for the writing thread
//! to wait for newly available space to write to, other than trying repeatedly.
//!
//! # Examples
//!
//! Moving single elements into and out of a queue with
//! [`Producer::push()`] and [`Consumer::pop()`], respectively:
//!
//! ```
//! use rtrb::{RingBuffer, PushError, PopError};
//!
//! let (mut producer, mut consumer) = RingBuffer::new(2);
//!
//! assert_eq!(producer.push(10), Ok(()));
//! assert_eq!(producer.push(20), Ok(()));
//! assert_eq!(producer.push(30), Err(PushError::Full(30)));
//!
//! std::thread::spawn(move || {
//!     assert_eq!(consumer.pop(), Ok(10));
//!     assert_eq!(consumer.pop(), Ok(20));
//!     assert_eq!(consumer.pop(), Err(PopError::Empty));
//! }).join().unwrap();
//! ```
//!
//! See the documentation of the [`chunks#examples`] module
//! for examples that write multiple items at once with
//! [`Producer::write_chunk_uninit()`] and [`Producer::write_chunk()`]
//! and read multiple items with [`Consumer::read_chunk()`].

#![cfg_attr(not(feature = "std"), no_std)]
#![warn(rust_2018_idioms)]
//#![deny(missing_docs, missing_debug_implementations)]
#![deny(unsafe_op_in_unsafe_fn)]
#![warn(clippy::undocumented_unsafe_blocks, clippy::unnecessary_safety_comment)]

extern crate alloc;

use alloc::boxed::Box;
use alloc::vec::Vec;
use core::marker::PhantomData;
use core::mem::{ManuallyDrop, MaybeUninit};
use core::ops::Deref;
use core::ptr::NonNull;
use core::sync::atomic::{AtomicU8, AtomicUsize, Ordering};

#[allow(dead_code, clippy::undocumented_unsafe_blocks)]
mod cache_padded;
use cache_padded::CachePadded;

//pub mod chunks;
pub mod diy;

//pub mod array;
//pub mod embedded;

#[cfg(feature = "mmap")]
pub mod mmap;

/*
// This is used in the documentation.
#[allow(unused_imports)]
use chunks::WriteChunkUninit;
*/

use diy::{update_capacity, Calc, Capacity, Flags, Indices, Storage};

// TODO: use errors::* or something?
pub use diy::{PeekError, PopError, PushError};

use diy::IS_ABANDONED;

// NB: non-public!
type RingBufferInner<T> = DynamicStorage<T, { Calc::Twice as u8 }, CachePaddedIndices>;

/// A bounded single-producer single-consumer (SPSC) queue.
///
/// Elements can be written with a [`Producer`] and read with a [`Consumer`],
/// both of which can be obtained with [`RingBuffer::new()`].
///
/// *See also the [crate-level documentation](crate).*
#[derive(Debug)]
pub struct RingBuffer<T>(PhantomData<T>);

impl<T> RingBuffer<T> {
    /// Creates a ring buffer with the given `capacity` and returns [`Producer`] and [`Consumer`].
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
    #[inline(always)]
    #[allow(clippy::new_ret_no_self)]
    #[must_use]
    pub fn new(capacity: usize) -> (Producer<T>, Consumer<T>) {
        let (p, c) = RingBufferInner::<T>::new(capacity);
        (Producer(p), Consumer(c))
    }
}

// TODO: move to different module?
#[derive(Debug, PartialEq, Eq)]
pub struct Ptr<S> {
    ptr: NonNull<S>,
    _marker: PhantomData<S>,
}

impl<const C: u8, S: Storage<C>> Ptr<S>
{
    fn new(storage: S) -> (diy::Producer<Self>, diy::Consumer<Self>) {
        // NB: We are assuming that IS_ABANDONED is unset.
        let ptr = Box::leak(Box::new(storage));
        // SAFETY: Pointer from Box is always non-null.
        let ptr = unsafe { NonNull::new_unchecked(ptr) };
        // SAFETY: Only a single instance of Producer is allowed.
        let p = unsafe {
            diy::Producer::new(Self {
                ptr,
                _marker: PhantomData,
            })
        };
        // SAFETY: Only a single instance of Consumer is allowed.
        let c = unsafe {
            diy::Consumer::new(Self {
                ptr,
                _marker: PhantomData,
            })
        };
        (p, c)
    }
}

impl<S> Drop for Ptr<S> {
    fn drop(&mut self) {
        // SAFETY: must point to initialized Storage.
        let flags: &AtomicU8 = unsafe { self.ptr.as_ref().flags() };
        // The "store" part of `fetch_or()` has to use `Release` to make sure that any previous writes
        // to the ring buffer happen before it (in the thread that drops first).
        // The "load" part can be `Relaxed` for the first thread,
        // but it must be `Acquire` for the second one (see below).
        if flags.fetch_or(IS_ABANDONED, Ordering::Release) & IS_ABANDONED == 0 {
            // The flag wasn't set before, so we are the first to drop our
            // producer/consumer and it should not be dropped yet.
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
            let _ = flags.load(Ordering::Acquire);
            // SAFETY: RingBuffer has been allocated with `Box`.
            unsafe {
                drop_slow(self.ptr);
            }
        }
    }
}

/// Non-inlined part of `Ptr::drop()`.
#[inline(never)]
unsafe fn drop_slow<S>(ptr: NonNull<S>) {
    // SAFETY: This is allowed because the storage has been allocated with `Box::new()`.
    unsafe {
        // Turn the pointer back into a `Box` and immediately drop it,
        // which deallocates the memory allocated in `Ptr::new()`.
        drop(Box::from_raw(ptr.as_ptr()));
    }
}

impl<const C: u8, S: Storage<C>> Deref for Ptr<S> {
    type Target = S;

    fn deref(&self) -> &Self::Target {
        // SAFETY: There are no mutable references.
        unsafe { self.ptr.as_ref() }
    }
}

/// Dynamic storage on the heap.
// Once the `adt_const_params` feature has been stabilized
// (https://github.com/rust-lang/rust/issues/95174),
// `u8` can be replaced by `Addressing`.
#[derive(Debug)]
pub struct DynamicStorage<T, const C: u8, I: Indices> {
    indices: I,

    flags: AtomicU8,

    /// The buffer holding slots.
    data_ptr: *mut T,

    capacity: usize,

    /// Indicates that dropping a `DynamicStorage` may drop elements of type `T`.
    _marker: PhantomData<T>,
}

/// `T` is not `Sync` because we never share it across threads.
// SAFETY: There is not mutable state (except for interior mutability).
unsafe impl<T: Send, const C: u8, I: Indices + Sync> Sync for DynamicStorage<T, C, I> {}

// NB: DynamicStorage doesn't need to be `Send` because it is never moved.

impl<T, const C: u8, I: Indices> DynamicStorage<T, C, I> {
    #[allow(clippy::new_ret_no_self)]
    #[must_use]
    pub fn new(
        capacity: usize,
    ) -> (
        diy::Producer<C, Ptr<C, DynamicStorage<T, C, I>>>,
        diy::Consumer<C, Ptr<C, DynamicStorage<T, C, I>>>,
    ) {
        let capacity = update_capacity::<C>(capacity);
        Ptr::new(Self {
            indices: I::INIT,
            flags: AtomicU8::new(0),
            data_ptr: ManuallyDrop::new(Vec::with_capacity(capacity)).as_mut_ptr(),
            capacity,
            _marker: PhantomData,
        })
    }
}

impl<T, const C: u8, I: Indices> PartialEq for DynamicStorage<T, C, I> {
    fn eq(&self, other: &Self) -> bool {
        core::ptr::eq(self, other)
    }
}

impl<T, const C: u8, I: Indices> Eq for DynamicStorage<T, C, I> {}

impl<T, const C: u8, I: Indices> Capacity for DynamicStorage<T, C, I> {
    #[inline(always)]
    fn capacity(&self) -> usize {
        self.capacity
    }
}

// SAFETY: all methods must be implemented correctly, or the whole thing is unsound
unsafe impl<T, const C: u8, I: Indices> Storage<C> for DynamicStorage<T, C, I> {
    type Item = T;
    type Indices = I;

    #[inline(always)]
    fn data_ptr(&self) -> *mut Self::Item {
        self.data_ptr
    }

    #[inline(always)]
    fn indices(&self) -> &Self::Indices {
        &self.indices
    }
}

impl<T, const C: u8, I: Indices> Flags for DynamicStorage<T, C, I> {
    #[inline(always)]
    fn flags(&self) -> &AtomicU8 {
        &self.flags
    }
}

/// Padded indices to avoid false sharing.
#[derive(Debug)]
pub struct CachePaddedIndices {
    /// The head of the queue.
    ///
    /// This integer is in range `0 .. 2 * capacity`.
    head: CachePadded<AtomicUsize>,

    /// The tail of the queue.
    ///
    /// This integer is in range `0 .. 2 * capacity`.
    tail: CachePadded<AtomicUsize>,
}

// SAFETY: all methods must be implemented correctly, or the whole thing is unsound
unsafe impl Indices for CachePaddedIndices {
    #[allow(clippy::declare_interior_mutable_const)]
    const INIT: Self = CachePaddedIndices {
        head: CachePadded::new(AtomicUsize::new(0)),
        tail: CachePadded::new(AtomicUsize::new(0)),
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

impl<T, const C: u8, I: Indices> Drop for DynamicStorage<T, C, I> {
    /// Drops all non-empty slots.
    fn drop(&mut self) {
        // SAFETY: this is called exactly once, no references to any elements exist anymore.
        unsafe { self.drop_all_elements() };

        // Finally, deallocate the buffer, but don't run any destructors.
        // SAFETY: data_ptr and capacity are still valid from the original initialization.
        unsafe { Vec::from_raw_parts(self.data_ptr, 0, self.capacity()) };
    }
}

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
/// multiple elements at once can be written with [`Producer::write_chunk()`]
/// and [`Producer::write_chunk_uninit()`].
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
pub struct Producer<T>(diy::Producer<Ptr<RingBufferInner<T>>>);

impl<T> Producer<T> {
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
    #[inline(always)]
    pub fn push(&mut self, value: T) -> Result<(), PushError<T>> {
        self.0.push(value)
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
    #[inline(always)]
    pub fn slots(&self) -> usize {
        self.0.slots()
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
    #[inline(always)]
    pub fn is_full(&self) -> bool {
        self.0.is_full()
    }

    /// Returns the total capacity of the queue.
    ///
    /// At any time, the capacity is subdivided into
    /// [`Producer::slots()`] available for writing and
    /// [`Consumer::slots()`] available for reading.
    ///
    /// # Examples
    ///
    /// ```
    /// use rtrb::RingBuffer;
    ///
    /// let (producer, consumer) = RingBuffer::<f32>::new(100);
    /// assert_eq!(producer.capacity(), 100);
    /// assert_eq!(consumer.capacity(), 100);
    /// ```
    #[inline(always)]
    pub fn capacity(&self) -> usize {
        self.0.capacity()
    }

    /*
    /// Returns `true` if the corresponding [`Consumer`] has been destroyed.
    ///
    /// Note that since Rust version 1.74.0, this is not synchronizing with the consumer thread
    /// anymore, see <https://github.com/mgeier/rtrb/issues/114>.
    /// In a future version of `rtrb`, the synchronizing behavior might be restored.
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
    ///     // This is needed since Rust 1.74.0, see https://github.com/mgeier/rtrb/issues/114:
    ///     std::sync::atomic::fence(std::sync::atomic::Ordering::Acquire);
    ///     // The consumer does definitely not exist anymore.
    /// }
    /// ```
    pub fn is_abandoned(&self) -> bool {
        Arc::strong_count(&self.buffer) < 2
    }
    */
}

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
/// multiple elements at once can be read with [`Consumer::read_chunk()`].
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
pub struct Consumer<T>(diy::Consumer<Ptr<RingBufferInner<T>>>);

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
    #[inline(always)]
    pub fn pop(&mut self) -> Result<T, PopError> {
        self.0.pop()
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
    #[inline(always)]
    pub fn peek(&self) -> Result<&T, PeekError> {
        self.0.peek()
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
    #[inline(always)]
    pub fn slots(&self) -> usize {
        self.0.slots()
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
    #[inline(always)]
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /*
    /// Returns `true` if the corresponding [`Producer`] has been destroyed.
    ///
    /// Note that since Rust version 1.74.0, this is not synchronizing with the producer thread
    /// anymore, see <https://github.com/mgeier/rtrb/issues/114>.
    /// In a future version of `rtrb`, the synchronizing behavior might be restored.
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
    ///     // This is needed since Rust 1.74.0, see https://github.com/mgeier/rtrb/issues/114:
    ///     std::sync::atomic::fence(std::sync::atomic::Ordering::Acquire);
    ///     // The producer does definitely not exist anymore.
    /// }
    /// ```
    #[inline(always)]
    pub fn is_abandoned(&self) -> bool {
        self.0.is_abandoned()
    }
    */

    /// Returns the total capacity of the queue.
    ///
    /// At any time, the capacity is subdivided into
    /// [`Producer::slots()`] available for writing and
    /// [`Consumer::slots()`] available for reading.
    ///
    /// # Examples
    ///
    /// ```
    /// use rtrb::RingBuffer;
    ///
    /// let (producer, consumer) = RingBuffer::<f32>::new(100);
    /// assert_eq!(producer.capacity(), 100);
    /// assert_eq!(consumer.capacity(), 100);
    /// ```
    #[inline(always)]
    pub fn capacity(&self) -> usize {
        self.0.capacity()
    }
}

/// Extension trait used to provide a [`copy_to_uninit()`](CopyToUninit::copy_to_uninit)
/// method on built-in slices.
///
/// This can be used to safely copy data to the slices returned from
/// [`WriteChunkUninit::as_mut_slices()`].
///
/// To use this, the trait has to be brought into scope, e.g. with:
///
/// ```
/// use rtrb::CopyToUninit;
/// ```
pub trait CopyToUninit<T: Copy> {
    /// Copies contents to a possibly uninitialized slice.
    fn copy_to_uninit<'a>(&self, dst: &'a mut [MaybeUninit<T>]) -> &'a mut [T];
}

impl<T: Copy> CopyToUninit<T> for [T] {
    /// Copies contents to a possibly uninitialized slice.
    ///
    /// # Panics
    ///
    /// This function will panic if the two slices have different lengths.
    fn copy_to_uninit<'a>(&self, dst: &'a mut [MaybeUninit<T>]) -> &'a mut [T] {
        assert_eq!(
            self.len(),
            dst.len(),
            "source slice length does not match destination slice length"
        );
        let dst_ptr = dst.as_mut_ptr().cast();
        // SAFETY: The lengths have been checked to be equal and
        // the mutable reference makes sure that there is no overlap.
        unsafe {
            self.as_ptr().copy_to_nonoverlapping(dst_ptr, self.len());
            core::slice::from_raw_parts_mut(dst_ptr, self.len())
        }
    }
}

/// Ring buffer with power-of-two storage.
// TODO: change to newtype, add docs
pub type RingBuffer2<T> = DynamicStorage<T, { Addressing::PowerOfTwo as u8 }, CachePaddedIndices>;

// TODO: Remove because power-of-two optimizations? Might be done automatically by the compiler?
// TODO: verify
pub type StaticRingBuffer2<T, const N: usize> =
    diy::ArrayStorage<T, N, { Addressing::PowerOfTwo as u8 }, CachePaddedIndices>;
