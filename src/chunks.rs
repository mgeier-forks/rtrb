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
//! use rtrb::{Producer, CopyToUninit as _};
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
//! use rtrb::{Producer, CopyToUninit as _, ChunkError::TooFewSlots};
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
//! use rtrb::{Producer, ChunkError::TooFewSlots};
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

// TODO: move to submodules, use correct RingBuffer variant.
// This is used in the documentation.
#[allow(unused_imports)]
use crate::{
    Consumer, CopyToUninit, Producer, ReadChunk, RingBuffer, WriteChunk, WriteChunkUninit,
};
