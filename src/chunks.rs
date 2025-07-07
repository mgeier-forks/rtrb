//! Writing and reading multiple items at once into and from a [`RingBuffer`].
//!
//! Multiple items at once can be moved from an iterator into the ring buffer by using
//! [`Producer::write_chunk_uninit()`] followed by [`WriteChunkUninit::fill_from_iter()`].
//! Alternatively, mutable access to the (uninitialized) slots of the chunk can be obtained with
//! [`WriteChunkUninit::as_mut_slices()`], which requires writing some `unsafe` code.
//! To avoid that, [`Producer::write_chunk()`] can be used,
//! which initializes all slots with their [`Default`] value
//! and provides mutable access by means of [`WriteChunk::as_mut_slices()`].
//!
//! Multiple items at once can be moved out of the ring buffer by using
//! [`Consumer::read_chunk()`] and iterating over the returned [`ReadChunk`]
//! (or by explicitly calling [`ReadChunk::into_iter()`]).
//! Immutable access to the slots of the chunk can be obtained with [`ReadChunk::as_slices()`].
//!
//! # Examples
//!
//! This example uses a single thread for simplicity, but in a real application,
//! `producer` and `consumer` would of course live on different threads:
//!
//! ```
//! use rtrb::RingBuffer;
//!
//! let (mut producer, mut consumer) = RingBuffer::new(5);
//!
//! if let Ok(chunk) = producer.write_chunk_uninit(4) {
//!     chunk.fill_from_iter([10, 11, 12]);
//!     // Note that we requested 4 slots but we've only written to 3 of them!
//! } else {
//!     unreachable!();
//! }
//!
//! assert_eq!(producer.slots(), 2);
//! assert_eq!(consumer.slots(), 3);
//!
//! if let Ok(chunk) = consumer.read_chunk(2) {
//!     assert_eq!(chunk.into_iter().collect::<Vec<_>>(), [10, 11]);
//! } else {
//!     unreachable!();
//! }
//!
//! // One element is still in the queue:
//! assert_eq!(consumer.peek(), Ok(&12));
//!
//! let data = vec![20, 21, 22, 23];
//! // NB: write_chunk_uninit() could be used for possibly better performance:
//! if let Ok(mut chunk) = producer.write_chunk(4) {
//!     let (first, second) = chunk.as_mut_slices();
//!     let mid = first.len();
//!     first.copy_from_slice(&data[..mid]);
//!     second.copy_from_slice(&data[mid..]);
//!     chunk.commit_all();
//! } else {
//!     unreachable!();
//! }
//!
//! assert!(producer.is_full());
//! assert_eq!(consumer.slots(), 5);
//!
//! let mut v = Vec::<i32>::with_capacity(5);
//! if let Ok(chunk) = consumer.read_chunk(5) {
//!     let (first, second) = chunk.as_slices();
//!     v.extend(first);
//!     v.extend(second);
//!     chunk.commit_all();
//! } else {
//!     unreachable!();
//! }
//! assert_eq!(v, [12, 20, 21, 22, 23]);
//! assert!(consumer.is_empty());
//! ```
//!
//! The iterator API can be used to move items from one ring buffer to another:
//!
//! ```
//! use rtrb::{Consumer, Producer};
//!
//! fn move_items<T>(src: &mut Consumer<T>, dst: &mut Producer<T>) -> usize {
//!     let n = src.slots().min(dst.slots());
//!     dst.write_chunk_uninit(n).unwrap().fill_from_iter(src.read_chunk(n).unwrap())
//! }
//! ```
//!
//! ## Common Access Patterns
//!
//! The following examples show the [`Producer`] side;
//! similar patterns can of course be used with [`Consumer::read_chunk()`] as well.
//! Furthermore, the examples use [`Producer::write_chunk_uninit()`],
//! along with a bit of `unsafe` code.
//! To avoid this, you can use [`Producer::write_chunk()`] instead,
//! which requires the trait bound `T: Default` and will lead to a small runtime overhead.
//!
//! Copy a whole slice of items into the ring buffer, but only if space permits
//! (if not, the entire input slice is returned as an error):
//!
//! ```
//! use rtrb::{Producer, CopyToUninit};
//!
//! fn push_entire_slice<'a, T>(queue: &mut Producer<T>, slice: &'a [T]) -> Result<(), &'a [T]>
//! where
//!     T: Copy,
//! {
//!     if let Ok(mut chunk) = queue.write_chunk_uninit(slice.len()) {
//!         let (first, second) = chunk.as_mut_slices();
//!         let mid = first.len();
//!         slice[..mid].copy_to_uninit(first);
//!         slice[mid..].copy_to_uninit(second);
//!         // SAFETY: All slots have been initialized
//!         unsafe { chunk.commit_all() };
//!         Ok(())
//!     } else {
//!         Err(slice)
//!     }
//! }
//! ```
//!
//! Copy as many items as possible from a given slice, returning the number of copied items:
//!
//! ```
//! use rtrb::{Producer, CopyToUninit, chunks::ChunkError::TooFewSlots};
//!
//! fn push_partial_slice<T>(queue: &mut Producer<T>, slice: &[T]) -> usize
//! where
//!     T: Copy,
//! {
//!     let mut chunk = match queue.write_chunk_uninit(slice.len()) {
//!         Ok(chunk) => chunk,
//!         // Remaining slots are returned, this will always succeed:
//!         Err(TooFewSlots(n)) => queue.write_chunk_uninit(n).unwrap(),
//!     };
//!     let end = chunk.len();
//!     let (first, second) = chunk.as_mut_slices();
//!     let mid = first.len();
//!     slice[..mid].copy_to_uninit(first);
//!     slice[mid..end].copy_to_uninit(second);
//!     // SAFETY: All slots have been initialized
//!     unsafe { chunk.commit_all() };
//!     end
//! }
//! ```
//!
//! Write as many slots as possible, given an iterator
//! (and return the number of written slots):
//!
//! ```
//! use rtrb::{Producer, chunks::ChunkError::TooFewSlots};
//!
//! fn push_from_iter<T, I>(queue: &mut Producer<T>, iter: I) -> usize
//! where
//!     T: Default,
//!     I: IntoIterator<Item = T>,
//! {
//!     let iter = iter.into_iter();
//!     let n = match iter.size_hint() {
//!         (_, None) => queue.slots(),
//!         (_, Some(n)) => n,
//!     };
//!     let chunk = match queue.write_chunk_uninit(n) {
//!         Ok(chunk) => chunk,
//!         // Remaining slots are returned, this will always succeed:
//!         Err(TooFewSlots(n)) => queue.write_chunk_uninit(n).unwrap(),
//!     };
//!     chunk.fill_from_iter(iter)
//! }
//! ```

use core::{fmt, mem::MaybeUninit};

use crate::{diy::Addressing, CachePaddedIndices, Consumer, DynamicStorage, Producer, Ptr};

// This is used in the documentation.
#[allow(unused_imports)]
use crate::{CopyToUninit, RingBuffer};

impl<T> Producer<T> {
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
    #[inline(always)]
    pub fn write_chunk(&mut self, n: usize) -> Result<WriteChunk<'_, T>, ChunkError>
    where
        T: Default,
    {
        self.0.write_chunk(n).map(WriteChunk)
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
    #[inline(always)]
    pub fn write_chunk_uninit(&mut self, n: usize) -> Result<WriteChunkUninit<'_, T>, ChunkError> {
        self.0.write_chunk_uninit(n).map(WriteChunkUninit)
    }
}

impl<T> Consumer<T> {
    /// Returns `n` slots for reading.
    ///
    /// [`ReadChunk::as_slices()`] provides immutable access to the slots.
    /// After reading from those slots, they explicitly have to be made available
    /// to be written again by the [`Producer`] by calling [`ReadChunk::commit()`]
    /// or [`ReadChunk::commit_all()`].
    ///
    /// Alternatively, items can be moved out of the [`ReadChunk`] using iteration
    /// because it implements [`IntoIterator`]
    /// ([`ReadChunk::into_iter()`] can be used to explicitly turn it into an [`Iterator`]).
    /// All moved items are automatically made available to be written again by the [`Producer`].
    ///
    /// # Errors
    ///
    /// If not enough slots are available, an error
    /// (containing the number of available slots) is returned.
    /// Use [`Consumer::slots()`] to obtain the number of available slots beforehand.
    ///
    /// # Examples
    ///
    /// See the documentation of the [`chunks`](crate::chunks#examples) module.
    #[inline(always)]
    pub fn read_chunk(&mut self, n: usize) -> Result<ReadChunk<'_, T>, ChunkError> {
        self.0.read_chunk(n).map(ReadChunk)
    }
}

/// Structure for writing into multiple ([`Default`]-initialized) slots in one go.
///
/// This is returned from [`Producer::write_chunk()`].
///
/// To obtain uninitialized slots, use [`Producer::write_chunk_uninit()`] instead,
/// which also allows moving items from an iterator into the ring buffer
/// by means of [`WriteChunkUninit::fill_from_iter()`].
pub struct WriteChunk<'a, T>(
    crate::diy::chunks::WriteChunk<
        'a,
        Ptr<DynamicStorage<T, { Addressing::Tight as u8 }, CachePaddedIndices>>,
    >,
);

impl<T> fmt::Debug for WriteChunk<'_, T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.pad("WriteChunk")
    }
}

impl<T> WriteChunk<'_, T>
where
    T: Default,
{
    /// Returns two slices for writing to the requested slots.
    ///
    /// All slots are initially filled with their [`Default`] value.
    ///
    /// The first slice can only be empty if `0` slots have been requested.
    /// If the first slice contains all requested slots, the second one is empty.
    ///
    /// After writing to the slots, they are *not* automatically made available
    /// to be read by the [`Consumer`].
    /// This has to be explicitly done by calling [`commit()`](WriteChunk::commit)
    /// or [`commit_all()`](WriteChunk::commit_all).
    /// If items are written but *not* committed afterwards,
    /// they will *not* become available for reading and
    /// they will be leaked (which is only relevant if `T` implements [`Drop`]).
    #[inline(always)]
    pub fn as_mut_slices(&mut self) -> (&mut [T], &mut [T]) {
        self.0.as_mut_slices()
    }

    /// Makes the first `n` slots of the chunk available for reading.
    ///
    /// The rest of the chunk is dropped.
    ///
    /// # Panics
    ///
    /// Panics if `n` is greater than the number of slots in the chunk.
    #[inline(always)]
    pub fn commit(self, n: usize) {
        self.0.commit(n)
    }

    /// Makes the whole chunk available for reading.
    #[inline(always)]
    pub fn commit_all(self) {
        self.0.commit_all()
    }

    /// Returns the number of slots in the chunk.
    #[must_use]
    #[inline(always)]
    pub fn len(&self) -> usize {
        self.0.len()
    }

    /// Returns `true` if the chunk contains no slots.
    #[must_use]
    #[inline(always)]
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

/// Structure for writing into multiple (uninitialized) slots in one go.
///
/// This is returned from [`Producer::write_chunk_uninit()`].
#[derive(PartialEq, Eq)]
pub struct WriteChunkUninit<'a, T>(
    crate::diy::chunks::WriteChunkUninit<
        'a,
        Ptr<DynamicStorage<T, { Addressing::Tight as u8 }, CachePaddedIndices>>,
    >,
);

impl<T> fmt::Debug for WriteChunkUninit<'_, T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.pad("WriteChunkUninit")
    }
}

impl<T> WriteChunkUninit<'_, T> {
    /// Returns two slices for writing to the requested slots.
    ///
    /// The first slice can only be empty if `0` slots have been requested.
    /// If the first slice contains all requested slots, the second one is empty.
    ///
    /// The extension trait [`CopyToUninit`] can be used to safely copy data into those slices.
    ///
    /// After writing to the slots, they are *not* automatically made available
    /// to be read by the [`Consumer`].
    /// This has to be explicitly done by calling [`commit()`](WriteChunkUninit::commit)
    /// or [`commit_all()`](WriteChunkUninit::commit_all).
    /// If items are written but *not* committed afterwards,
    /// they will *not* become available for reading and
    /// they will be leaked (which is only relevant if `T` implements [`Drop`]).
    #[inline(always)]
    pub fn as_mut_slices(&mut self) -> (&mut [MaybeUninit<T>], &mut [MaybeUninit<T>]) {
        self.0.as_mut_slices()
    }

    /// Makes the first `n` slots of the chunk available for reading.
    ///
    /// # Panics
    ///
    /// Panics if `n` is greater than the number of slots in the chunk.
    ///
    /// # Safety
    ///
    /// The caller must make sure that the first `n` elements have been initialized.
    #[inline(always)]
    pub unsafe fn commit(self, n: usize) {
        // SAFETY: See docstring.
        unsafe { self.0.commit(n) }
    }

    /// Makes the whole chunk available for reading.
    ///
    /// # Safety
    ///
    /// The caller must make sure that all elements have been initialized.
    #[inline(always)]
    pub unsafe fn commit_all(self) {
        // SAFETY: See docstring.
        unsafe { self.0.commit_all() }
    }

    /// Moves items from an iterator into the (uninitialized) slots of the chunk.
    ///
    /// The number of moved items is returned.
    ///
    /// All moved items are automatically made availabe to be read by the [`Consumer`].
    ///
    /// # Examples
    ///
    /// If the iterator contains too few items, only a part of the chunk
    /// is made available for reading:
    ///
    /// ```
    /// use rtrb::{RingBuffer, PopError};
    ///
    /// let (mut p, mut c) = RingBuffer::new(4);
    ///
    /// if let Ok(chunk) = p.write_chunk_uninit(3) {
    ///     assert_eq!(chunk.fill_from_iter([10, 20]), 2);
    /// } else {
    ///     unreachable!();
    /// }
    /// assert_eq!(p.slots(), 2);
    /// assert_eq!(c.pop(), Ok(10));
    /// assert_eq!(c.pop(), Ok(20));
    /// assert_eq!(c.pop(), Err(PopError::Empty));
    /// ```
    ///
    /// If the chunk size is too small, some items may remain in the iterator.
    /// To be able to keep using the iterator after the call,
    /// `&mut` (or [`Iterator::by_ref()`]) can be used.
    ///
    /// ```
    /// use rtrb::{RingBuffer, PopError};
    ///
    /// let (mut p, mut c) = RingBuffer::new(4);
    ///
    /// let mut it = vec![10, 20, 30].into_iter();
    /// if let Ok(chunk) = p.write_chunk_uninit(2) {
    ///     assert_eq!(chunk.fill_from_iter(&mut it), 2);
    /// } else {
    ///     unreachable!();
    /// }
    /// assert_eq!(c.pop(), Ok(10));
    /// assert_eq!(c.pop(), Ok(20));
    /// assert_eq!(c.pop(), Err(PopError::Empty));
    /// assert_eq!(it.next(), Some(30));
    /// ```
    #[inline(always)]
    pub fn fill_from_iter<I>(self, iter: I) -> usize
    where
        I: IntoIterator<Item = T>,
    {
        self.0.fill_from_iter(iter)
    }

    /// Returns the number of slots in the chunk.
    #[must_use]
    #[inline(always)]
    pub fn len(&self) -> usize {
        self.0.len()
    }

    /// Returns `true` if the chunk contains no slots.
    #[must_use]
    #[inline(always)]
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

/// Structure for reading from multiple slots in one go.
///
/// This is returned from [`Consumer::read_chunk()`].
#[derive(Debug, PartialEq, Eq)]
pub struct ReadChunk<'a, T>(
    crate::diy::chunks::ReadChunk<
        'a,
        Ptr<DynamicStorage<T, { Addressing::Tight as u8 }, CachePaddedIndices>>,
    >,
);

impl<T> ReadChunk<'_, T> {
    /// Returns two slices for reading from the requested slots.
    ///
    /// The first slice can only be empty if `0` slots have been requested.
    /// If the first slice contains all requested slots, the second one is empty.
    ///
    /// The provided slots are *not* automatically made available
    /// to be written again by the [`Producer`].
    /// This has to be explicitly done by calling [`commit()`](ReadChunk::commit)
    /// or [`commit_all()`](ReadChunk::commit_all).
    /// Note that this runs the destructor of the committed items (if `T` implements [`Drop`]).
    /// You can "peek" at the contained values by simply not calling any of the "commit" methods.
    #[must_use]
    #[inline(always)]
    pub fn as_slices(&self) -> (&[T], &[T]) {
        self.0.as_slices()
    }

    /// Returns two mutable slices for reading from the requested slots.
    ///
    /// This has the same semantics as [`as_slices()`](ReadChunk::as_slices),
    /// except that it returns mutable slices and requires a mutable reference
    /// to the chunk.
    ///
    /// In the vast majority of cases, mutable access is not required when
    /// reading data and the immutable version should be preferred. However,
    /// there are some scenarios where it might be desirable to perform
    /// operations on the data in-place without copying it to a separate buffer
    /// (e.g. streaming decryption), in which case this version can be used.
    #[must_use]
    #[inline(always)]
    pub fn as_mut_slices(&mut self) -> (&mut [T], &mut [T]) {
        self.0.as_mut_slices()
    }

    /// Drops the first `n` slots of the chunk, making the space available for writing again.
    ///
    /// # Panics
    ///
    /// Panics if `n` is greater than the number of slots in the chunk.
    ///
    /// # Examples
    ///
    /// The following example shows that items are dropped when "committed"
    /// (which is only relevant if `T` implements [`Drop`]).
    ///
    /// ```
    /// use rtrb::RingBuffer;
    ///
    /// // Static variable to count all drop() invocations
    /// static mut DROP_COUNT: i32 = 0;
    /// #[derive(Debug)]
    /// struct Thing;
    /// impl Drop for Thing {
    ///     fn drop(&mut self) { unsafe { DROP_COUNT += 1; } }
    /// }
    ///
    /// // Scope to limit lifetime of ring buffer
    /// {
    ///     let (mut p, mut c) = RingBuffer::new(2);
    ///
    ///     assert!(p.push(Thing).is_ok()); // 1
    ///     assert!(p.push(Thing).is_ok()); // 2
    ///     if let Ok(thing) = c.pop() {
    ///         // "thing" has been *moved* out of the queue but not yet dropped
    ///         assert_eq!(unsafe { DROP_COUNT }, 0);
    ///     } else {
    ///         unreachable!();
    ///     }
    ///     // First Thing has been dropped when "thing" went out of scope:
    ///     assert_eq!(unsafe { DROP_COUNT }, 1);
    ///     assert!(p.push(Thing).is_ok()); // 3
    ///
    ///     if let Ok(chunk) = c.read_chunk(2) {
    ///         assert_eq!(chunk.len(), 2);
    ///         assert_eq!(unsafe { DROP_COUNT }, 1);
    ///         chunk.commit(1); // Drops only one of the two Things
    ///         assert_eq!(unsafe { DROP_COUNT }, 2);
    ///     } else {
    ///         unreachable!();
    ///     }
    ///     // The last Thing is still in the queue ...
    ///     assert_eq!(unsafe { DROP_COUNT }, 2);
    /// }
    /// // ... and it is dropped when the ring buffer goes out of scope:
    /// assert_eq!(unsafe { DROP_COUNT }, 3);
    /// ```
    #[inline(always)]
    pub fn commit(self, n: usize) {
        self.0.commit(n)
    }

    /// Drops all slots of the chunk, making the space available for writing again.
    #[inline(always)]
    pub fn commit_all(self) {
        self.0.commit_all()
    }

    /// Returns the number of slots in the chunk.
    #[must_use]
    #[inline(always)]
    pub fn len(&self) -> usize {
        self.0.len()
    }

    /// Returns `true` if the chunk contains no slots.
    #[must_use]
    #[inline(always)]
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

impl<'a, T> IntoIterator for ReadChunk<'a, T> {
    type Item = T;
    type IntoIter = ReadChunkIntoIter<'a, T>;

    /// Turns a [`ReadChunk`] into an iterator.
    ///
    /// When the iterator is dropped, all iterated slots are made available for writing again.
    /// Non-iterated items remain in the ring buffer.
    fn into_iter(self) -> Self::IntoIter {
        ReadChunkIntoIter(self.0.into_iter())
    }
}

/// An iterator that moves out of a [`ReadChunk`].
///
/// This `struct` is created by the [`into_iter()`](ReadChunk::into_iter) method
/// on [`ReadChunk`] (provided by the [`IntoIterator`] trait).
///
/// When this `struct` is dropped, the iterated slots are made available for writing again.
/// Non-iterated items remain in the ring buffer.
pub struct ReadChunkIntoIter<'a, T>(
    crate::diy::chunks::ReadChunkIntoIter<
        'a,
        Ptr<DynamicStorage<T, { Addressing::Tight as u8 }, CachePaddedIndices>>,
    >,
);

impl<T> fmt::Debug for ReadChunkIntoIter<'_, T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.pad("ReadChunkIntoIter")
    }
}

impl<T> Iterator for ReadChunkIntoIter<'_, T> {
    type Item = T;

    #[inline(always)]
    fn next(&mut self) -> Option<Self::Item> {
        self.0.next()
    }

    #[inline(always)]
    fn size_hint(&self) -> (usize, Option<usize>) {
        self.0.size_hint()
    }
}

impl<T> ExactSizeIterator for ReadChunkIntoIter<'_, T> {}

impl<T> core::iter::FusedIterator for ReadChunkIntoIter<'_, T> {}

#[cfg(feature = "std")]
impl std::io::Write for Producer<u8> {
    #[inline]
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        use ChunkError::TooFewSlots;
        let mut chunk = match self.write_chunk_uninit(buf.len()) {
            Ok(chunk) => chunk,
            Err(TooFewSlots(0)) => return Err(std::io::ErrorKind::WouldBlock.into()),
            Err(TooFewSlots(n)) => self.write_chunk_uninit(n).unwrap(),
        };
        let end = chunk.len();
        let (first, second) = chunk.as_mut_slices();
        let mid = first.len();
        // NB: If buf.is_empty(), chunk will be empty as well and the following are no-ops:
        buf[..mid].copy_to_uninit(first);
        buf[mid..end].copy_to_uninit(second);
        // SAFETY: All slots have been initialized
        unsafe { chunk.commit_all() };
        Ok(end)
    }

    #[inline]
    fn flush(&mut self) -> std::io::Result<()> {
        // Nothing to do here.
        Ok(())
    }
}

#[cfg(feature = "std")]
impl std::io::Read for Consumer<u8> {
    #[inline]
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        use ChunkError::TooFewSlots;
        let chunk = match self.read_chunk(buf.len()) {
            Ok(chunk) => chunk,
            Err(TooFewSlots(0)) => return Err(std::io::ErrorKind::WouldBlock.into()),
            Err(TooFewSlots(n)) => self.read_chunk(n).unwrap(),
        };
        let (first, second) = chunk.as_slices();
        let mid = first.len();
        let end = chunk.len();
        // NB: If buf.is_empty(), chunk will be empty as well and the following are no-ops:
        buf[..mid].copy_from_slice(first);
        buf[mid..end].copy_from_slice(second);
        chunk.commit_all();
        Ok(end)
    }
}

/// Error type for [`Consumer::read_chunk()`], [`Producer::write_chunk()`]
/// and [`Producer::write_chunk_uninit()`].
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum ChunkError {
    /// Fewer than the requested number of slots were available.
    ///
    /// Contains the number of slots that were available.
    TooFewSlots(usize),
}

#[cfg(feature = "std")]
impl std::error::Error for ChunkError {}

impl fmt::Display for ChunkError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ChunkError::TooFewSlots(n) => {
                alloc::format!("only {} slots available in ring buffer", n).fmt(f)
            }
        }
    }
}
