use core::cell::{Cell, UnsafeCell};
use core::fmt;
use core::mem::MaybeUninit;
use core::ops::Deref;
use core::sync::atomic::{AtomicU8, AtomicUsize, Ordering};

pub mod chunks;

// TODO: separate module for traits?

pub const HAS_PRODUCER: u8 = 0b10000000;
pub const HAS_CONSUMER: u8 = 0b01000000;
// NB: This overlaps with HAS_PRODUCER, they are never used at the same time.
pub const IS_ABANDONED: u8 = 0b10000000;

/// Indices.
///
/// # Safety
///
/// The indices must not be changed by anyone else.
// TODO: this is not really something the implementer can control!
pub unsafe trait Indices {
    const INIT: Self;

    fn head(&self) -> &AtomicUsize;
    fn tail(&self) -> &AtomicUsize;
}

/// Different index calculations.
#[repr(u8)]
pub enum Calc {
    /// Indices are wrapped at twice the buffer size.
    DoubleSize,
    /// Indices are wrapped at [`usize::MAX`].
    PowerOfTwo,
    // TODO: SingleSize, reduce capacity by 1
    // TODO: SingleSizePowerOfTwo?
}

impl Calc {
    // This is a work-around until the `adt_const_params` feature has been stabilized
    // (https://github.com/rust-lang/rust/issues/95174):
    pub const fn from_u8(value: u8) -> Calc {
        if value == Calc::DoubleSize as u8 {
            Calc::DoubleSize
        } else if value == Calc::PowerOfTwo as u8 {
            Calc::PowerOfTwo
        } else {
            panic!("Invalid value for Calc")
        }
    }

    /// Some index calculations need to extend the capacity.
    pub const fn update_capacity(&self, capacity: usize) -> usize {
        // TODO: check whether number range is large enough for twice the buffer size

        // MSRV 1.46: match statements in const fn
        match self {
            Calc::DoubleSize => capacity,
            Calc::PowerOfTwo => capacity.next_power_of_two(),
        }
    }
}

pub trait IndexCalculation {
    const CALC: Calc;

    fn capacity(&self) -> usize;

    #[inline]
    fn collapse_position(&self, pos: usize) -> usize {
        match Self::CALC {
            Calc::DoubleSize => {
                // Wraps a position from the range `0 .. 2 * capacity` to `0 .. capacity`.
                debug_assert!(pos == 0 || pos < 2 * self.capacity());
                if pos < self.capacity() {
                    pos
                } else {
                    pos - self.capacity()
                }
            }
            Calc::PowerOfTwo => {
                // Wraps from any number to the range `0 .. capacity`.
                // TODO: is capacity 0 supported?
                pos & (self.capacity() - 1)
            }
        }
    }

    /// Increments a position by going `n` slots forward.
    #[inline]
    fn increment(&self, pos: usize, n: usize) -> usize {
        match Self::CALC {
            Calc::DoubleSize => {
                debug_assert!(pos == 0 || pos < 2 * self.capacity());
                debug_assert!(n <= self.capacity());
                let threshold = 2 * self.capacity() - n;
                if pos < threshold {
                    pos + n
                } else {
                    pos - threshold
                }
            }
            Calc::PowerOfTwo => pos.wrapping_add(n),
        }
    }

    /// Increments a position by going one slot forward.
    ///
    /// This might be more efficient than self.increment(..., 1).
    #[inline]
    fn increment1(&self, pos: usize) -> usize {
        match Self::CALC {
            Calc::DoubleSize => {
                debug_assert_ne!(self.capacity(), 0);
                debug_assert!(pos < 2 * self.capacity());
                if pos < 2 * self.capacity() - 1 {
                    pos + 1
                } else {
                    0
                }
            }
            Calc::PowerOfTwo => pos.wrapping_add(1),
        }
    }

    /// Returns the distance between two positions.
    #[inline]
    fn distance(&self, a: usize, b: usize) -> usize {
        match Self::CALC {
            Calc::DoubleSize => {
                debug_assert!(a == 0 || a < 2 * self.capacity());
                debug_assert!(b == 0 || b < 2 * self.capacity());
                if a <= b {
                    b - a
                } else {
                    2 * self.capacity() - a + b
                }
            }
            Calc::PowerOfTwo => b.wrapping_sub(a),
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
///
/// Several functions must not be exposed to the user: indices(), flags(), ...
pub unsafe trait Storage: IndexCalculation {
    type Item;
    type Indices: Indices;

    // TODO: make sure head/tail are not exposed to the user?
    fn indices(&self) -> &Self::Indices;

    // TODO: make "unsafe"?
    // TODO: make "container" that contains Storage and other traits without exposing them.
    fn flags(&self) -> &AtomicU8;

    fn data_ptr(&self) -> *mut Self::Item;

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
    ///
    /// The threads must have been synchronized before via `flags()`.
    #[inline(never)]
    unsafe fn drop_all_elements(&mut self) {
        // These atomic variables are *not* used for synchronizing the threads
        // before destruction.  Relaxed ordering is sufficient here.
        let mut head = self.indices().head().load(Ordering::Relaxed);
        let tail = self.indices().tail().load(Ordering::Relaxed);

        // Loop over all slots that hold a value and drop them.
        while head != tail {
            // SAFETY: All slots between head and tail have been initialized.
            unsafe { self.slot_ptr(head).drop_in_place() };
            head = self.increment1(head);
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
        // SAFETY: See docstring.
        unsafe { self.data_ptr().add(self.collapse_position(pos)) }
    }
}

#[derive(Debug, PartialEq, Eq)]
// NB: this syntax needs MSRV 1.79
//pub struct Producer<R: Deref<Target: Storage>>
pub struct Producer<R: Deref>
where
    R::Target: Storage,
{
    /// A reference to the ring buffer.
    buffer: R,

    /// A copy of `buffer.head` for quick access.
    ///
    /// This value can be stale and sometimes needs to be resynchronized with `buffer.head`.
    cached_head: Cell<usize>,

    /// A copy of `buffer.tail` for quick access.
    ///
    /// This value is always in sync with `buffer.tail`.
    cached_tail: Cell<usize>,
}

/// It (and any wrapper structs) can be moved ...
/// ```
/// fn assert_send<X: Send>() {}
/// assert_send::<rtrb::Producer<u8>>();
/// ```
/// ... but not shared between threads:
/// ```compile_fail
/// fn assert_sync<X: Sync>() {}
/// assert_sync::<rtrb::Producer<u8>>();
/// ```
// SAFETY: After moving a producer to another thread, there is still only a single thread
// that can access the producer side of the queue.
unsafe impl<S: Storage, R: Deref<Target = S>> Send for Producer<R>
where
    S: Sync,
    S::Item: Send,
{
}

impl<S: Storage, R: Deref<Target = S>> Producer<R> {
    /// Create a new producer.
    ///
    /// # Safety
    ///
    /// Only a single `Producer` can exist at a time.
    pub unsafe fn new(buffer: R) -> Self {
        let head = buffer.indices().head().load(Ordering::Acquire);
        let tail = buffer.indices().tail().load(Ordering::Acquire);
        Self {
            buffer,
            cached_head: Cell::new(head),
            cached_tail: Cell::new(tail),
        }
    }

    pub fn push(&mut self, value: S::Item) -> Result<(), PushError<S::Item>> {
        if let Some(tail) = self.next_tail() {
            let b = &self.buffer;
            // SAFETY: tail points to an empty slot.
            unsafe { b.slot_ptr(tail).write(value) };
            let tail = b.increment1(tail);
            b.indices().tail().store(tail, Ordering::Release);
            self.cached_tail.set(tail);
            Ok(())
        } else {
            Err(PushError::Full(value))
        }
    }

    pub fn slots(&self) -> usize {
        let b = &self.buffer;
        let head = b.indices().head().load(Ordering::Acquire);
        self.cached_head.set(head);
        b.capacity() - b.distance(head, self.cached_tail.get())
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
    /// For performance, this special case is implemented separately.
    #[inline]
    fn next_tail(&self) -> Option<usize> {
        let head = self.cached_head.get();
        let tail = self.cached_tail.get();
        let b = &self.buffer;
        // Check if the queue is *possibly* full.
        if b.distance(head, tail) == b.capacity() {
            // Refresh the head ...
            let head = b.indices().head().load(Ordering::Acquire);
            self.cached_head.set(head);
            // ... and check if it's *really* full.
            if b.distance(head, tail) == b.capacity() {
                // `head` didn't change, queue is full.
                return None;
            }
        }
        Some(tail)
    }
}

impl<R: Deref> Drop for Producer<R>
where
    R::Target: Storage,
{
    fn drop(&mut self) {
        // SAFETY: This is only called in Producer::drop().
        unsafe { self.buffer.drop_producer() };
    }
}

#[derive(Debug, PartialEq, Eq)]
pub struct Consumer<R: Deref>
where
    R::Target: Storage,
{
    /// A reference to the ring buffer.
    buffer: R,

    /// A copy of `buffer.head` for quick access.
    ///
    /// This value is always in sync with `buffer.head`.
    cached_head: Cell<usize>,

    /// A copy of `buffer.tail` for quick access.
    ///
    /// This value can be stale and sometimes needs to be resynchronized with `buffer.tail`.
    cached_tail: Cell<usize>,
}

/// It (and any wrapper structs) can be moved ...
/// ```
/// fn assert_send<X: Send>() {}
/// assert_send::<rtrb::Consumer<u8>>();
/// ```
/// ... but not shared between threads:
/// ```compile_fail
/// fn assert_sync<X: Sync>() {}
/// assert_sync::<rtrb::Consumer<u8>>();
/// ```
// SAFETY: After moving a Consumer to another thread, there is still only a single thread
// that can access the consumer side of the queue.
unsafe impl<S: Storage, R: Deref<Target = S>> Send for Consumer<R>
where
    S: Sync,
    S::Item: Send,
{
}

impl<S: Storage, R: Deref<Target = S>> Consumer<R> {
    /// Create a new consumer.
    ///
    /// # Safety
    ///
    /// Only a single `Consumer` can exist at a time.
    pub unsafe fn new(buffer: R) -> Self {
        let head = buffer.indices().head().load(Ordering::Acquire);
        let tail = buffer.indices().tail().load(Ordering::Acquire);
        Self {
            buffer,
            cached_head: Cell::new(head),
            cached_tail: Cell::new(tail),
        }
    }

    pub fn pop(&mut self) -> Result<S::Item, PopError> {
        if let Some(head) = self.next_head() {
            let b = &self.buffer;
            // SAFETY: head points to an initialized slot.
            let value = unsafe { b.slot_ptr(head).read() };
            let head = b.increment1(head);
            b.indices().head().store(head, Ordering::Release);
            self.cached_head.set(head);
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
        let b = &self.buffer;
        let tail = b.indices().tail().load(Ordering::Acquire);
        self.cached_tail.set(tail);
        b.distance(self.cached_head.get(), tail)
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
    /// For performance, this special case is implemented separately.
    #[inline]
    fn next_head(&self) -> Option<usize> {
        let head = self.cached_head.get();
        let tail = self.cached_tail.get();

        // Check if the queue is *possibly* empty.
        if head == tail {
            // Refresh the tail ...
            let tail = self.buffer.indices().tail().load(Ordering::Acquire);
            self.cached_tail.set(tail);
            // ... and check if it's *really* empty.
            if head == tail {
                // `tail` didn't change, queue is empty.
                return None;
            }
        }
        Some(head)
    }
}

impl<R: Deref> Drop for Consumer<R>
where
    R::Target: Storage,
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

/// Storage in a (compile-time sized) array.
// Once the `adt_const_params` feature has been stabilized
// (https://github.com/rust-lang/rust/issues/95174),
// `u8` can be replaced by `Addressing`.
#[derive(Debug)]
pub struct ArrayStorage<T, const N: usize, const C: u8, I: Indices> {
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

/// `T` is not `Sync` because we never share it across threads.
unsafe impl<T: Send, const N: usize, const C: u8, I: Indices + Sync> Sync
    for ArrayStorage<T, N, C, I>
{
}

unsafe impl<T: Send, const N: usize, const C: u8, I: Indices + Send> Send
    for ArrayStorage<T, N, C, I>
{
}

impl<T, const N: usize, const C: u8, I: Indices> ArrayStorage<T, N, C, I> {
    pub const fn new() -> Self {
        const {
            // assert!() in const since Rust 1.57
            assert!(
                Calc::from_u8(C).update_capacity(N) == N,
                "`capacity` must be a power of two"
            );
        }
        Self {
            indices: I::INIT,
            flags: AtomicU8::new(0),
            slots: UnsafeCell::new([const { MaybeUninit::uninit() }; N]),
        }
    }

    pub fn producer(&self) -> Option<Producer<&Self>> {
        let old_flags = self.flags().fetch_or(HAS_PRODUCER, Ordering::SeqCst);
        if old_flags & HAS_PRODUCER == 0 {
            // SAFETY: This is the one and only producer.
            Some(unsafe { Producer::new(self) })
        } else {
            None
        }
    }

    pub fn consumer(&self) -> Option<Consumer<&Self>> {
        let old_flags = self.flags().fetch_or(HAS_CONSUMER, Ordering::SeqCst);
        if old_flags & HAS_CONSUMER == 0 {
            // SAFETY: This is the one and only consumer.
            Some(unsafe { Consumer::new(self) })
        } else {
            None
        }
    }
}

impl<T, const N: usize, const C: u8, I: Indices> Drop for ArrayStorage<T, N, C, I> {
    fn drop(&mut self) {
        // SAFETY: this is called exactly once, no references to any elements exist anymore.
        unsafe { self.drop_all_elements() };
    }
}

impl<T, const N: usize, const C: u8, I: Indices> Default for ArrayStorage<T, N, C, I> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T, const N: usize, const C: u8, I: Indices> PartialEq for ArrayStorage<T, N, C, I> {
    fn eq(&self, other: &Self) -> bool {
        core::ptr::eq(self, other)
    }
}

impl<T, const N: usize, const C: u8, I: Indices> Eq for ArrayStorage<T, N, C, I> {}

impl<T, const N: usize, const C: u8, I: Indices> IndexCalculation for ArrayStorage<T, N, C, I> {
    const CALC: Calc = Calc::from_u8(C);

    #[inline(always)]
    fn capacity(&self) -> usize {
        N
    }
}

// SAFETY: all methods must be implemented correctly, or the whole thing is unsound
unsafe impl<T, const N: usize, const C: u8, I: Indices> Storage for ArrayStorage<T, N, C, I> {
    type Item = T;
    type Indices = I;

    #[inline(always)]
    fn flags(&self) -> &AtomicU8 {
        &self.flags
    }

    #[inline(always)]
    fn data_ptr(&self) -> *mut Self::Item {
        // TODO: what happens if N == 0?
        self.slots.get().cast()
    }

    #[inline(always)]
    fn indices(&self) -> &Self::Indices {
        &self.indices
    }

    #[inline(always)]
    unsafe fn drop_producer(&self) {
        let _ = self.flags().fetch_and(!HAS_PRODUCER, Ordering::SeqCst);
    }

    #[inline(always)]
    unsafe fn drop_consumer(&self) {
        let _ = self.flags().fetch_and(!HAS_CONSUMER, Ordering::SeqCst);
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
