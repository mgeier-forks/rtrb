use core::{marker::PhantomData, mem::MaybeUninit, ops::Deref, sync::atomic::Ordering};

use super::{Consumer, IndexCalculation as _, Indices, Producer, Storage};
use crate::chunks::ChunkError;

#[derive(PartialEq, Eq)]
pub struct WriteChunkUninit<'a, R: Deref>
where
    R::Target: Storage,
{
    first_ptr: *mut <R::Target as Storage>::Item,
    first_len: usize,
    second_ptr: *mut <R::Target as Storage>::Item,
    second_len: usize,
    producer: &'a Producer<R>,
}

/// It (and any wrapper structs) can be moved ...
/// ```
/// fn assert_send<X: Send>() {}
/// assert_send::<rtrb::chunks::WriteChunkUninit<u8>>();
/// ```
/// ... but not shared between threads:
/// ```compile_fail
/// fn assert_sync<X: Sync>() {}
/// assert_sync::<rtrb::chunks::WriteChunkUninit<u8>>();
/// ```
// SAFETY: WriteChunkUninit only exists while a unique reference to the producer is held.
// It is therefore safe to move it to another thread.
unsafe impl<S: Storage, R: Deref<Target = S>> Send for WriteChunkUninit<'_, R> where S::Item: Send {}

impl<S: Storage + ?Sized, R: Deref<Target = S>> WriteChunkUninit<'_, R> {
    #[allow(clippy::type_complexity)]
    pub fn as_mut_slices(&mut self) -> (&mut [MaybeUninit<S::Item>], &mut [MaybeUninit<S::Item>]) {
        // SAFETY: The pointers and lengths have been computed correctly in write_chunk_uninit().
        unsafe {
            (
                core::slice::from_raw_parts_mut(self.first_ptr.cast(), self.first_len),
                core::slice::from_raw_parts_mut(self.second_ptr.cast(), self.second_len),
            )
        }
    }

    /// # Safety
    ///
    /// TODO: refer to rtrb::RingBuffer
    pub unsafe fn commit(self, n: usize) {
        assert!(n <= self.len(), "cannot commit more than chunk size");
        // SAFETY: Delegated to the caller.
        unsafe { self.commit_unchecked(n) };
    }

    /// # Safety
    ///
    /// TODO: refer to rtrb::RingBuffer
    pub unsafe fn commit_all(self) {
        let slots = self.len();
        // SAFETY: Delegated to the caller.
        unsafe { self.commit_unchecked(slots) };
    }

    unsafe fn commit_unchecked(self, n: usize) -> usize {
        let p = self.producer;
        let tail = p.buffer.increment(p.cached_tail.get(), n);
        p.buffer.indices().tail().store(tail, Ordering::Release);
        p.cached_tail.set(tail);
        n
    }

    pub fn fill_from_iter<I>(self, iter: I) -> usize
    where
        I: IntoIterator<Item = <R::Target as Storage>::Item>,
    {
        let mut iter = iter.into_iter();
        let mut iterated = 0;
        'outer: for &(ptr, len) in &[
            (self.first_ptr, self.first_len),
            (self.second_ptr, self.second_len),
        ] {
            for i in 0..len {
                match iter.next() {
                    Some(item) => {
                        // SAFETY: It is allowed to write to this memory slot
                        unsafe { ptr.add(i).write(item) };
                        iterated += 1;
                    }
                    None => break 'outer,
                }
            }
        }
        // SAFETY: iterated slots have been initialized above
        unsafe { self.commit_unchecked(iterated) }
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.first_len + self.second_len
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.first_len == 0
    }

    /// Drops all elements starting from index `n`.
    ///
    /// #Safety
    ///
    /// All of those slots must be initialized.
    unsafe fn drop_suffix(&mut self, n: usize) {
        // NB: If n >= self.len(), the loops are not entered.
        for i in n..self.first_len {
            // SAFETY: The caller must make sure that all slots are initialized.
            unsafe { self.first_ptr.add(i).drop_in_place() };
        }
        for i in n.saturating_sub(self.first_len)..self.second_len {
            // SAFETY: The caller must make sure that all slots are initialized.
            unsafe { self.second_ptr.add(i).drop_in_place() };
        }
    }
}

pub struct WriteChunk<'a, R: Deref>(Option<WriteChunkUninit<'a, R>>)
where
    R::Target: Storage;

impl<R: Deref> Drop for WriteChunk<'_, R>
where
    R::Target: Storage,
{
    fn drop(&mut self) {
        // NB: If `commit()` or `commit_all()` has been called, `self.0` is `None`.
        if let Some(mut chunk) = self.0.take() {
            // No part of the chunk has been committed, all slots are dropped.
            // SAFETY: All slots have been initialized in From::from().
            unsafe { chunk.drop_suffix(0) };
        }
    }
}

impl<'a, S: Storage, R: Deref<Target = S>> From<WriteChunkUninit<'a, R>> for WriteChunk<'a, R>
where
    S::Item: Default,
{
    /// Fills all slots with the [`Default`] value.
    fn from(chunk: WriteChunkUninit<'a, R>) -> Self {
        for i in 0..chunk.first_len {
            // SAFETY: i is in a valid range.
            unsafe { chunk.first_ptr.add(i).write(Default::default()) };
        }
        for i in 0..chunk.second_len {
            // SAFETY: i is in a valid range.
            unsafe { chunk.second_ptr.add(i).write(Default::default()) };
        }
        WriteChunk(Some(chunk))
    }
}

impl<S: Storage, R: Deref<Target = S>> WriteChunk<'_, R>
where
    S::Item: Default,
{
    pub fn as_mut_slices(&mut self) -> (&mut [S::Item], &mut [S::Item]) {
        // self.0 is always Some(chunk).
        let chunk = self.0.as_ref().unwrap();
        // SAFETY: The pointers and lengths have been computed correctly in write_chunk_uninit()
        // and all slots have been initialized in From::from().
        unsafe {
            (
                core::slice::from_raw_parts_mut(chunk.first_ptr, chunk.first_len),
                core::slice::from_raw_parts_mut(chunk.second_ptr, chunk.second_len),
            )
        }
    }

    pub fn commit(mut self, n: usize) {
        // self.0 is always Some(chunk).
        let mut chunk = self.0.take().unwrap();
        // SAFETY: All slots have been initialized in From::from().
        unsafe {
            // Slots at index `n` and higher are dropped ...
            chunk.drop_suffix(n);
            // ... everything below `n` is committed.
            chunk.commit(n);
        }
        // `self` is dropped here, with `self.0` being set to `None`.
    }

    pub fn commit_all(mut self) {
        // self.0 is always Some(chunk).
        let chunk = self.0.take().unwrap();
        // SAFETY: All slots have been initialized in From::from().
        unsafe { chunk.commit_all() };
        // `self` is dropped here, with `self.0` being set to `None`.
    }

    pub fn len(&self) -> usize {
        // self.0 is always Some(chunk).
        self.0.as_ref().unwrap().len()
    }

    pub fn is_empty(&self) -> bool {
        // self.0 is always Some(chunk).
        self.0.as_ref().unwrap().is_empty()
    }
}

pub trait ReadChunk<'a, R: Deref>
where
    R::Target: Storage,
{
    /// Creates a new chunk for reading.
    ///
    /// # Safety
    ///
    /// The given index and length must point to initialized slots.
    unsafe fn new(consumer: &'a Consumer<R>, head: usize, len: usize) -> Self;
}

/// A chunk for reading.
///
///
#[derive(Debug, PartialEq, Eq)]
pub struct ReadChunkTwoSlices<'a, R: Deref>
where
    R::Target: Storage,
{
    // Must be "mut" for drop_in_place()
    first_ptr: *mut <R::Target as Storage>::Item,
    first_len: usize,
    // Must be "mut" for drop_in_place()
    second_ptr: *mut <R::Target as Storage>::Item,
    second_len: usize,
    consumer: &'a Consumer<R>,
    /// Indicates that dropping a `ReadChunkTwoSlices` may drop elements of type `R::Target::Item`.
    _marker: PhantomData<<R::Target as Storage>::Item>,
}

/// It (and any wrapper structs) can be moved ...
/// ```
/// fn assert_send<X: Send>() {}
/// assert_send::<rtrb::chunks::ReadChunk<u8>>();
/// ```
/// ... but not shared between threads:
/// ```compile_fail
/// fn assert_sync<X: Sync>() {}
/// assert_sync::<rtrb::chunks::ReadChunk<u8>>();
/// ```
// SAFETY: ReadChunkTwoSlices only exists while a unique reference to the consumer is held.
// It is therefore safe to move it to another thread.
unsafe impl<S: Storage, R: Deref<Target = S>> Send for ReadChunkTwoSlices<'_, R> where S::Item: Send {}

impl<'a, S: Storage, R: Deref<Target = S>> ReadChunk<'a, R> for ReadChunkTwoSlices<'a, R> {
    // ...
    //
    // # Safety
    //
    // ...
    unsafe fn new(consumer: &'a Consumer<R>, head: usize, len: usize) -> Self {
        let b = &consumer.buffer;
        let first_len = len.min(b.capacity() - head);
        Self {
            // SAFETY: Caller has to guarantee correct input values.
            first_ptr: unsafe { b.data_ptr().add(head) },
            first_len,
            second_ptr: b.data_ptr(),
            second_len: len - first_len,
            consumer,
            _marker: PhantomData,
        }
    }
}

impl<S: Storage, R: Deref<Target = S>> ReadChunkTwoSlices<'_, R> {
    pub fn as_slices(&self) -> (&[S::Item], &[S::Item]) {
        // SAFETY: The pointers and lengths have been computed correctly in read_chunk().
        unsafe {
            (
                core::slice::from_raw_parts(self.first_ptr, self.first_len),
                core::slice::from_raw_parts(self.second_ptr, self.second_len),
            )
        }
    }

    pub fn as_mut_slices(&mut self) -> (&mut [S::Item], &mut [S::Item]) {
        // SAFETY: The pointers and lengths have been computed correctly in read_chunk().
        unsafe {
            (
                core::slice::from_raw_parts_mut(self.first_ptr, self.first_len),
                core::slice::from_raw_parts_mut(self.second_ptr, self.second_len),
            )
        }
    }

    pub fn commit(self, n: usize) {
        assert!(n <= self.len(), "cannot commit more than chunk size");
        // SAFETY: self.len() initialized elements have been obtained in read_chunk().
        unsafe { self.commit_unchecked(n) };
    }

    pub fn commit_all(self) {
        let slots = self.len();
        // SAFETY: self.len() initialized elements have been obtained in read_chunk().
        unsafe { self.commit_unchecked(slots) };
    }

    unsafe fn commit_unchecked(self, n: usize) -> usize {
        let first_len = self.first_len.min(n);
        for i in 0..first_len {
            // SAFETY: The caller must make sure that there are n initialized elements.
            unsafe { self.first_ptr.add(i).drop_in_place() };
        }
        let second_len = self.second_len.min(n - first_len);
        for i in 0..second_len {
            // SAFETY: The caller must make sure that there are n initialized elements.
            unsafe { self.second_ptr.add(i).drop_in_place() };
        }
        let c = self.consumer;
        let head = c.buffer.increment(c.cached_head.get(), n);
        c.buffer.indices().head().store(head, Ordering::Release);
        c.cached_head.set(head);
        n
    }

    pub fn len(&self) -> usize {
        self.first_len + self.second_len
    }

    pub fn is_empty(&self) -> bool {
        self.first_len == 0
    }
}

impl<'a, S: Storage, R: Deref<Target = S>> IntoIterator for ReadChunkTwoSlices<'a, R> {
    type Item = S::Item;
    type IntoIter = ReadChunkTwoSlicesIntoIter<'a, R>;

    fn into_iter(self) -> Self::IntoIter {
        Self::IntoIter {
            chunk: self,
            iterated: 0,
        }
    }
}

pub struct ReadChunkTwoSlicesIntoIter<'a, R: Deref>
where
    R::Target: Storage,
{
    chunk: ReadChunkTwoSlices<'a, R>,
    iterated: usize,
}

impl<R: Deref> Drop for ReadChunkTwoSlicesIntoIter<'_, R>
where
    R::Target: Storage,
{
    /// Makes all iterated slots available for writing again.
    ///
    /// Non-iterated items remain in the ring buffer and are *not* dropped.
    fn drop(&mut self) {
        let c = self.chunk.consumer;
        let head = c.buffer.increment(c.cached_head.get(), self.iterated);
        c.buffer.indices().head().store(head, Ordering::Release);
        c.cached_head.set(head);
    }
}

impl<S: Storage, R: Deref<Target = S>> Iterator for ReadChunkTwoSlicesIntoIter<'_, R> {
    type Item = S::Item;

    fn next(&mut self) -> Option<Self::Item> {
        let ptr = if self.iterated < self.chunk.first_len {
            // SAFETY: first_len is valid.
            unsafe { self.chunk.first_ptr.add(self.iterated) }
        } else if self.iterated < self.chunk.first_len + self.chunk.second_len {
            // SAFETY: first_len and second_len are valid.
            unsafe {
                self.chunk
                    .second_ptr
                    .add(self.iterated - self.chunk.first_len)
            }
        } else {
            return None;
        };
        self.iterated += 1;
        // SAFETY: ptr points to an initialized slot.
        Some(unsafe { ptr.read() })
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let remaining = self.chunk.first_len + self.chunk.second_len - self.iterated;
        (remaining, Some(remaining))
    }
}

impl<S: Storage, R: Deref<Target = S>> ExactSizeIterator for ReadChunkTwoSlicesIntoIter<'_, R> {}

impl<S: Storage, R: Deref<Target = S>> core::iter::FusedIterator
    for ReadChunkTwoSlicesIntoIter<'_, R>
{
}

#[derive(Debug, PartialEq, Eq)]
pub struct ReadChunkOneSlice<'a, R: Deref>
where
    R::Target: Storage,
{
    // Must be "mut" for drop_in_place()
    ptr: *mut <R::Target as Storage>::Item,
    len: usize,
    consumer: &'a Consumer<R>,
    /// Indicates that dropping a `ReadChunkOneSlice` may drop elements of type `R::Target::Item`.
    _marker: PhantomData<<R::Target as Storage>::Item>,
}

// SAFETY: ReadChunkOneSlice only exists while a unique reference to the consumer is held.
// It is therefore safe to move it to another thread.
unsafe impl<S: Storage, R: Deref<Target = S>> Send for ReadChunkOneSlice<'_, R> where S::Item: Send {}

impl<'a, S: Storage, R: Deref<Target = S>> ReadChunk<'a, R> for ReadChunkOneSlice<'a, R> {
    // ...
    //
    // # Safety
    //
    // ...
    unsafe fn new(consumer: &'a Consumer<R>, head: usize, len: usize) -> Self {
        let b = &consumer.buffer;
        Self {
            // SAFETY: Caller has to guarantee correct input values.
            ptr: unsafe { b.data_ptr().add(head) },
            len,
            consumer,
            _marker: PhantomData,
        }
    }
}

impl<S: Storage, R: Deref<Target = S>> ReadChunkOneSlice<'_, R> {
    pub fn as_slice(&self) -> &[S::Item] {
        // SAFETY: The correct pointer and length have been provided by ReadChunkOneSlice::new().
        unsafe { core::slice::from_raw_parts(self.ptr, self.len) }
    }

    pub fn as_mut_slice(&mut self) -> &mut [S::Item] {
        // SAFETY: The correct pointer and length have been provided by ReadChunkOneSlice::new().
        unsafe { core::slice::from_raw_parts_mut(self.ptr, self.len) }
    }

    pub fn commit(self, n: usize) {
        assert!(n <= self.len(), "cannot commit more than chunk size");
        // SAFETY: self.len() initialized elements have been obtained in read_chunk().
        unsafe { self.commit_unchecked(n) };
    }

    pub fn commit_all(self) {
        let slots = self.len();
        // SAFETY: self.len() initialized elements have been obtained in read_chunk().
        unsafe { self.commit_unchecked(slots) };
    }

    unsafe fn commit_unchecked(self, n: usize) -> usize {
        let len = self.len.min(n);
        for i in 0..len {
            // SAFETY: The caller must make sure that there are n initialized elements.
            unsafe { self.ptr.add(i).drop_in_place() };
        }
        let c = self.consumer;
        let head = c.buffer.increment(c.cached_head.get(), n);
        c.buffer.indices().head().store(head, Ordering::Release);
        c.cached_head.set(head);
        n
    }

    pub fn len(&self) -> usize {
        self.len
    }

    pub fn is_empty(&self) -> bool {
        self.len == 0
    }
}

impl<'a, S: Storage, R: Deref<Target = S>> IntoIterator for ReadChunkOneSlice<'a, R> {
    type Item = S::Item;
    type IntoIter = ReadChunkOneSliceIntoIter<'a, R>;

    fn into_iter(self) -> Self::IntoIter {
        Self::IntoIter {
            chunk: self,
            iterated: 0,
        }
    }
}

pub struct ReadChunkOneSliceIntoIter<'a, R: Deref>
where
    R::Target: Storage,
{
    chunk: ReadChunkOneSlice<'a, R>,
    iterated: usize,
}

impl<R: Deref> Drop for ReadChunkOneSliceIntoIter<'_, R>
where
    R::Target: Storage,
{
    /// Makes all iterated slots available for writing again.
    ///
    /// Non-iterated items remain in the ring buffer and are *not* dropped.
    fn drop(&mut self) {
        let c = self.chunk.consumer;
        let head = c.buffer.increment(c.cached_head.get(), self.iterated);
        c.buffer.indices().head().store(head, Ordering::Release);
        c.cached_head.set(head);
    }
}

impl<S: Storage, R: Deref<Target = S>> Iterator for ReadChunkOneSliceIntoIter<'_, R> {
    type Item = S::Item;

    fn next(&mut self) -> Option<Self::Item> {
        let ptr = if self.iterated < self.chunk.len {
            // SAFETY: len is valid.
            unsafe { self.chunk.ptr.add(self.iterated) }
        } else {
            return None;
        };
        self.iterated += 1;
        // SAFETY: ptr points to an initialized slot.
        Some(unsafe { ptr.read() })
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let remaining = self.chunk.len - self.iterated;
        (remaining, Some(remaining))
    }
}

impl<S: Storage, R: Deref<Target = S>> ExactSizeIterator for ReadChunkOneSliceIntoIter<'_, R> {}

impl<S: Storage, R: Deref<Target = S>> core::iter::FusedIterator
    for ReadChunkOneSliceIntoIter<'_, R>
{
}

impl<S: Storage, R: Deref<Target = S>> Producer<R> {
    pub fn write_chunk_uninit(&mut self, n: usize) -> Result<WriteChunkUninit<'_, R>, ChunkError> {
        let head = self.cached_head.get();
        let tail = self.cached_tail.get();
        let b = &self.buffer;
        // Check if the queue has *possibly* not enough slots.
        if b.capacity() - b.distance(head, tail) < n {
            // Refresh the head ...
            let head = b.indices().head().load(Ordering::Acquire);
            self.cached_head.set(head);
            // ... and check if there *really* are not enough slots.
            let slots = b.capacity() - b.distance(head, tail);
            if slots < n {
                return Err(ChunkError::TooFewSlots(slots));
            }
        }
        let tail = b.collapse_position(tail);
        let first_len = n.min(b.capacity() - tail);
        Ok(WriteChunkUninit {
            // SAFETY: tail has been updated to a valid position.
            first_ptr: unsafe { b.data_ptr().add(tail) },
            first_len,
            second_ptr: b.data_ptr(),
            second_len: n - first_len,
            producer: self,
        })
    }

    pub fn write_chunk(&mut self, n: usize) -> Result<WriteChunk<'_, R>, ChunkError>
    where
        S::Item: Default,
    {
        self.write_chunk_uninit(n).map(WriteChunk::from)
    }
}

impl<S: Storage, R: Deref<Target = S>> Consumer<R> {
    /// Prepares a chunk for reading.
    ///
    /// # Safety
    ///
    /// The type `C` must be appropriate.
    pub unsafe fn read_chunk<'a, C: ReadChunk<'a, R>>(
        &'a mut self,
        n: usize,
    ) -> Result<C, ChunkError> {
        let head = self.cached_head.get();
        let tail = self.cached_tail.get();
        let b = &self.buffer;
        // Check if the queue has *possibly* not enough slots.
        if b.distance(head, tail) < n {
            // Refresh the tail ...
            let tail = b.indices().tail().load(Ordering::Acquire);
            self.cached_tail.set(tail);
            // ... and check if there *really* are not enough slots.
            let slots = b.distance(head, tail);
            if slots < n {
                return Err(ChunkError::TooFewSlots(slots));
            }
        }
        let head = b.collapse_position(head);
        // SAFETY: Index and size have been calculated correctly,
        // the caller must make sure that an appropriate `C` type is used.
        Ok(unsafe { C::new(self, head, n) })
    }
}
