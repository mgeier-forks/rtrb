macro_rules! ring_buffer {
    (
        storage = $storage:ident,
        N = $N:ident,
        arc = $arc:ident,
        bip = $bip:ident,
        contiguous = $contiguous:ident,
        padded = $padded:ident,
        pow2 = $pow2:ident,
        module = $module:literal,
        rb_doc = $rb_doc:expr
    ) => {


        #[cfg(feature = "std")]
        impl_!(Producer<u8>, trait = std::io::Write, N = $N, arc = $arc, {
            fn_producer_write!(contiguous = $contiguous);
            fn_producer_flush!();
        });

        #[cfg(feature = "std")]
        impl_!(Consumer<u8>, trait = std::io::Read, N = $N, arc = $arc, {
            fn_consumer_read!(contiguous = $contiguous);
        });

        impl_debug!(N = $N, RingBuffer);
        impl_debug!(N = $N, arc = $arc, Producer, Consumer);

        #[doc = mod_chunks_docstring!(storage = $storage, N = $N, arc = $arc, bip = $bip, contiguous = $contiguous, module = $module)]
        pub mod chunks {
            use core::{fmt, mem::MaybeUninit};
            use $crate::atomic::*;
            use super::{Consumer, Producer};
            // Only used in documentation:
            #[allow(unused_imports)]
            use super::CopyToUninit;

            struct_write_chunk_uninit!(N = $N, arc = $arc, contiguous = $contiguous);
            struct_write_chunk!(N = $N);
            struct_read_chunk!(N = $N, arc = $arc, contiguous = $contiguous);

            impl_send_for_chunks!(N = $N, module = $module);

            impl_!(WriteChunkUninit<'_>, N = $N, {
                fn_write_chunk_uninit_as_mut_sliceX!(contiguous = $contiguous);
                fn_write_chunk_uninit_commit_all!();
                fn_write_chunk_uninit_commit!();
                fn_write_chunk_uninit_fill_from_iter!(N = $N, arc = $arc, contiguous = $contiguous, module = $module);
                fn_X_chunk_X_len_and_is_empty!(contiguous = $contiguous);

                fn_write_chunk_uninit_drop_suffix!(contiguous = $contiguous);
                fn_write_chunk_uninit_commit_unchecked!(bip = $bip);
            });

            impl_!(WriteChunk<'_>, N = $N, {
                fn_write_chunk_as_mut_sliceX!(contiguous = $contiguous);
                fn_write_chunk_commit_all!();
                fn_write_chunk_commit!();
                fn_write_chunk_len_and_is_empty!();
            });

            impl_!(ReadChunk<'_>, N = $N, {
                fn_read_chunk_as_sliceX!(contiguous = $contiguous);
                fn_read_chunk_as_mut_sliceX!(contiguous = $contiguous);
                fn_read_chunk_commit_all!();
                fn_read_chunk_commit!(N = $N, arc = $arc, module = $module);
                fn_X_chunk_X_len_and_is_empty!(contiguous = $contiguous);

                fn_read_chunk_commit_unchecked!(contiguous = $contiguous);
            });

            impl_debug!(N = $N,
                WriteChunkUninit<'_>, WriteChunk<'_>, ReadChunk<'_>, ReadChunkIntoIter<'_>);

            impl_into_iterator_for_read_chunk!(N = $N, contiguous = $contiguous);
        }

        use chunks::{ReadChunk, WriteChunk, WriteChunkUninit};
    };
}

macro_rules! docstring {
    () => { "" };
    (#[doc = $first:expr] $(#[doc = $rest:expr])*) => {
        concat!($first, $("\n", $rest,)*)
    };
}

macro_rules! docstring_exclude_vrb {
    (vrb, $(#[doc = $line:expr])+) => { "" };
    ($storage:ident, $(#[doc = $line:expr])+) => { docstring!($(#[doc = $line])+) };
}

macro_rules! doctest_import {
    ($module:literal, $items:literal$(, $prefix:literal)?) => {
        concat!($($prefix, )?"use ", $module, "::", $items, ";")
    };
}

macro_rules! struct_ {
    ($(#[$attr:meta])* $vis:vis $name:ident<'a>, N = yes, $($body:tt)+) => {
        $(#[$attr])* $vis struct $name<'a, T, const N: usize> $($body)+
    };
    ($(#[$attr:meta])* $vis:vis $name:ident<'a>, N = no, $($body:tt)+) => {
        $(#[$attr])* $vis struct $name<'a, T> $($body)+
    };
    ($(#[$attr:meta])* $vis:vis $name:ident, N = yes, $($body:tt)+) => {
        $(#[$attr])* $vis struct $name<T, const N: usize> $($body)+
    };
    ($(#[$attr:meta])* $vis:vis $name:ident, N = no, $($body:tt)+) => {
        $(#[$attr])* $vis struct $name<T> $($body)+
    };
}

// $body can start with a "where" clause, if needed.
// "unsafe" is in a somewhat strange position to avoid parsing ambiguities.
macro_rules! impl_ {
    ($(#[$attr:meta])* $name:ident, $(trait $($unsafe:ident)? = $trait:ty,)? N = yes, arc = yes, $($body:tt)+) => {
        $(#[$attr])* $($($unsafe)?)? impl<T, const N: usize> $($trait for)? $name<T, N> $($body)+
    };
    ($(#[$attr:meta])* $name:ident, $(trait $($unsafe:ident)? = $trait:ty,)? N = no, arc = yes, $($body:tt)+) => {
        $(#[$attr])* $($($unsafe)?)? impl<T> $($trait for)? $name<T> $($body)+
    };
    ($(#[$attr:meta])* $name:ident, $(trait $($unsafe:ident)? = $trait:ty,)? N = yes, arc = no, $($body:tt)+) => {
        $(#[$attr])* $($($unsafe)?)? impl<T, const N: usize> $($trait for)? $name<'_, T, N> $($body)+
    };
    ($(#[$attr:meta])* $name:ident, $(trait $($unsafe:ident)? = $trait:ty,)? N = no, arc = no, $($body:tt)+) => {
        $(#[$attr])* $($($unsafe)?)? impl<T> $($trait for)? $name<'_, T> $($body)+
    };
    ($(#[$attr:meta])* $name:ident, $(trait $($unsafe:ident)? = $trait:ty,)? N = yes, $($body:tt)+) => {
        $(#[$attr])* $($($unsafe)?)? impl<T, const N: usize> $($trait for)? $name<T, N> $($body)+
    };
    ($(#[$attr:meta])* $name:ident, $(trait $($unsafe:ident)? = $trait:ty,)? N = no, $($body:tt)+) => {
        $(#[$attr])* $($($unsafe)?)? impl<T> $($trait for)? $name<T> $($body)+
    };
    ($(#[$attr:meta])* $name:ident<'_>, $(trait $($unsafe:ident)? = $trait:ty,)? N = yes, $($body:tt)+) => {
        $(#[$attr])* $($($unsafe)?)? impl<T, const N: usize> $($trait for)? $name<'_, T, N> $($body)+
    };
    ($(#[$attr:meta])* $name:ident<'_>, $(trait $($unsafe:ident)? = $trait:ty,)? N = no, $($body:tt)+) => {
        $($($unsafe)?)? impl<T> $($trait for)? $name<'_, T> $($body)+
    };
    ($(#[$attr:meta])* $name:ident<'a>, $(trait $($unsafe:ident)? = $trait:ty,)? N = yes, $($body:tt)+) => {
        $(#[$attr])* $($($unsafe)?)? impl<'a, T, const N: usize> $($trait for)? $name<'a, T, N> $($body)+
    };
    ($(#[$attr:meta])* $name:ident<'a>, $(trait $($unsafe:ident)? = $trait:ty,)? N = no, $($body:tt)+) => {
        $(#[$attr])* $($($unsafe)?)? impl<'a, T> $($trait for)? $name<'a, T> $($body)+
    };
    ($(#[$attr:meta])* $name:ident<$ty:ty>, $(trait $($unsafe:ident)? = $trait:ty,)? N = yes, arc = yes, $($body:tt)+) => {
        $(#[$attr])* $($($unsafe)?)? impl<const N: usize> $($trait for)? $name<$ty, N> $($body)+
    };
    ($(#[$attr:meta])* $name:ident<$ty:ty>, $(trait $($unsafe:ident)? = $trait:ty,)? N = no, arc = yes, $($body:tt)+) => {
        $(#[$attr])* $($($unsafe)?)? impl $($trait for)? $name<$ty> $($body)+
    };
    ($(#[$attr:meta])* $name:ident<$ty:ty>, $(trait $($unsafe:ident)? = $trait:ty,)? N = yes, arc = no, $($body:tt)+) => {
        $(#[$attr])* $($($unsafe)?)? impl<const N: usize> $($trait for)? $name<'_, $ty, N> $($body)+
    };
    ($(#[$attr:meta])* $name:ident<$ty:ty>, $(trait $($unsafe:ident)? = $trait:ty,)? N = no, arc = no, $($body:tt)+) => {
        $(#[$attr])* $($($unsafe)?)? impl $($trait for)? $name<'_, $ty> $($body)+
    };
}

macro_rules! generic {
    ($name:ident, N = yes, arc = yes) => {
        $name<T, N>
    };
    ($name:ident, N = no, arc = yes) => {
        $name<T>
    };
    ($name:ident, N = yes, arc = no) => {
        $name<'a, T, N>
    };
    ($name:ident, N = no, arc = no) => {
        $name<'a, T>
    };
    ($name:ident, N = yes) => {
        $name<T, N>
    };
    ($name:ident, N = no) => {
        $name<T>
    };
    ($name:ident<'_>, N = yes) => {
        $name<'_, T, N>
    };
    ($name:ident<'_>, N = no) => {
        $name<'_, T>
    };
    ($name:ident<'a>, N = yes) => {
        $name<'a, T, N>
    };
    ($name:ident<'a>, N = no) => {
        $name<'a, T>
    };
}

#[cfg(feature = "std")]
macro_rules! fn_producer_write {
    (contiguous = yes) => {
        #[inline]
        fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
            use ChunkError::TooFewSlots;
            let mut chunk = match self.write_chunk_uninit(buf.len()) {
                Ok(chunk) => chunk,
                Err(TooFewSlots(0)) => return Err(std::io::ErrorKind::WouldBlock.into()),
                Err(TooFewSlots(n)) => self.write_chunk_uninit(n).unwrap(),
            };
            let end = chunk.len();
            // NB: If buf.is_empty(), chunk will be empty as well and the following are no-ops:
            buf[..end].copy_to_uninit(chunk.as_mut_slice());
            // SAFETY: All slots have been initialized
            unsafe { chunk.commit_all() };
            Ok(end)
        }
    };
    (contiguous = no) => {
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
    };
}

#[cfg(feature = "std")]
macro_rules! fn_producer_flush {
    () => {
        #[inline]
        fn flush(&mut self) -> std::io::Result<()> {
            // Nothing to do here.
            Ok(())
        }
    };
}

#[cfg(feature = "std")]
macro_rules! fn_consumer_read {
    (contiguous = yes) => {
        #[inline]
        fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
            use ChunkError::TooFewSlots;
            let chunk = match self.read_chunk(buf.len()) {
                Ok(chunk) => chunk,
                Err(TooFewSlots(0)) => return Err(std::io::ErrorKind::WouldBlock.into()),
                Err(TooFewSlots(n)) => self.read_chunk(n).unwrap(),
            };
            let end = chunk.len();
            // NB: If buf.is_empty(), chunk will be empty as well and the following are no-ops:
            buf[..end].copy_from_slice(chunk.as_slice());
            chunk.commit_all();
            Ok(end)
        }
    };
    (contiguous = no) => {
        #[inline]
        fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
            use ChunkError::TooFewSlots;
            let chunk = match self.read_chunk(buf.len()) {
                Ok(chunk) => chunk,
                Err(TooFewSlots(0)) => return Err(std::io::ErrorKind::WouldBlock.into()),
                Err(TooFewSlots(n)) => self.read_chunk(n).unwrap(),
            };
            let end = chunk.len();
            let (first, second) = chunk.as_slices();
            let mid = first.len();
            // NB: If buf.is_empty(), chunk will be empty as well and the following are no-ops:
            buf[..mid].copy_from_slice(first);
            buf[mid..end].copy_from_slice(second);
            chunk.commit_all();
            Ok(end)
        }
    };
}

macro_rules! fn_write_chunk_uninit_commit_unchecked {
    (bip = yes) => {
        unsafe fn commit_unchecked(self, n: usize) -> usize {
            if n == 0 {
                // NB: No slots will be skipped, both `tail` and `skip` remain unchanged.
                // This is the same as if the function wasn't called at all.
                return n;
            }
            let b = &self.producer.buffer;
            let mut tail = self.producer.cached_tail.get();
            let collapsed_tail = b.collapse_position(tail);
            if self.ptr == b.data_ptr() && collapsed_tail != 0 {
                // We are writing a chunk at the beginning of the buffer
                // but the write index is not at the beginning!
                // This means we have skipped some slots and have to
                // set `skip` and fast-forward `tail`.

                // NB: It is safe to store `skip` before `tail`, because the consumer
                // will potentially only read between `head` and (the old) `tail`,
                // without looking at `skip`.
                // Storing `tail` before `skip` would be problematic, however, because
                // the consumer would see new data at the beginning of the buffer,
                // but wouldn't know that the end has to be skipped.
                b.skip.store(collapsed_tail, Ordering::Release);
                tail = b.increment(tail, b.capacity() - collapsed_tail + n);
                debug_assert_eq!(b.collapse_position(tail), n);
            } else {
                tail = b.increment(tail, n);
            }
            b.tail.store(tail, Ordering::Release);
            self.producer.cached_tail.set(tail);
            n
        }
    };
    (bip = no) => {
        unsafe fn commit_unchecked(self, n: usize) -> usize {
            let p = self.producer;
            let tail = p.buffer.increment(p.cached_tail.get(), n);
            p.buffer.tail.store(tail, Ordering::Release);
            p.cached_tail.set(tail);
            n
        }
    };
}

macro_rules! fn_read_chunk_commit_unchecked {
    (contiguous = yes) => {
        unsafe fn commit_unchecked(self, n: usize) -> usize {
            for i in 0..n {
                // SAFETY: The caller must make sure that there are n initialized elements.
                unsafe { self.ptr.add(i).drop_in_place() };
            }
            let b = &self.consumer.buffer;
            let head = b.increment(self.consumer.cached_head.get(), n);
            b.head.store(head, Ordering::Release);
            self.consumer.cached_head.set(head);
            n
        }
    };
    (contiguous = no) => {
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
            c.buffer.head.store(head, Ordering::Release);
            c.cached_head.set(head);
            n
        }
    };
}

macro_rules! fn_producer_write_chunk_uninit_docstring {
    (bip = $bip:ident, contiguous = $contiguous:ident) => { docstring!(
        /// Prepares a chunk of `n` (uninitialized) slots for writing.
        ///
        #[doc = choice!($contiguous,
            /// [`WriteChunkUninit::as_mut_slice()`]
            ::
            /// [`WriteChunkUninit::as_mut_slices()`]
        )]
        /// provides mutable access
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
        /// Use
        #[doc = choice!($bip,
            /// [`slots_contiguous_max()`](Producer::slots_contiguous_max) (or
            /// [`slots_contiguous_first()`](Producer::slots_contiguous_first))
            ::
            /// [`slots()`](Producer::slots)
        )]
        /// to obtain the number of available slots beforehand.
        ///
        /// # Safety
        ///
        /// This function itself is safe, as is [`WriteChunkUninit::fill_from_iter()`].
        /// However, when using
        #[doc = choice!($contiguous,
            /// [`WriteChunkUninit::as_mut_slice()`],
            ::
            /// [`WriteChunkUninit::as_mut_slices()`],
        )]
        /// the user has to make sure that the relevant slots have been initialized
        /// before calling [`WriteChunkUninit::commit()`] or [`WriteChunkUninit::commit_all()`].
        ///
        /// For a safe alternative that provides
        #[doc = choice!($contiguous, "a mutable slice", "mutable slices")]
        /// of [`Default`]-initialized slots, see [`Producer::write_chunk()`].
    ) };
}

macro_rules! fn_producer_write_chunk_uninit {
    (N = $N:ident, bip = yes, contiguous = $contiguous:ident) => {
        #[doc = fn_producer_write_chunk_uninit_docstring!(bip = yes, contiguous = $contiguous)]
        pub fn write_chunk_uninit(
            &mut self,
            n: usize,
        ) -> Result<generic!(WriteChunkUninit<'_>, N = $N), ChunkError> {
            let b = &self.buffer;
            let (mut slots, refreshed, try_at_beginning) = self.slots_contiguous_helper();
            if slots >= n {
                let offset = b.collapse_position(self.cached_tail.get());
                // SAFETY: `offset` has been set to a valid position.
                return Ok(unsafe { WriteChunkUninit::new(self, n, offset) });
            } else if try_at_beginning {
                // TODO: get refreshed_head.or_else(self.cached_head.get)
                let mut head = self.cached_head.get();
                let mut collapsed_head = b.collapse_position(head);
                if collapsed_head >= n {
                    // SAFETY: 0 is a valid position.
                    return Ok(unsafe { WriteChunkUninit::new(self, n, 0) });
                }
                // TODO: refreshed_head.is_none()
                if !refreshed {
                    head = b.head.load(Ordering::Acquire);
                    self.cached_head.set(head);
                    collapsed_head = b.collapse_position(head);
                    if collapsed_head >= n {
                        // SAFETY: 0 is a valid position.
                        return Ok(unsafe { WriteChunkUninit::new(self, n, 0) });
                    }
                }
                slots = slots.max(collapsed_head);
            }
            Err(ChunkError::TooFewSlots(slots))
        }
    };
    (N = $N:ident, bip = no, contiguous = $contiguous:ident) => {
        #[doc = fn_producer_write_chunk_uninit_docstring!(bip = no, contiguous = $contiguous)]
        pub fn write_chunk_uninit(
            &mut self,
            n: usize,
        ) -> Result<generic!(WriteChunkUninit<'_>, N = $N), ChunkError> {
            let head = self.cached_head.get();
            let tail = self.cached_tail.get();
            let b = &self.buffer;
            // Check if the queue has *possibly* not enough slots.
            if b.capacity() - b.distance(head, tail) < n {
                // Refresh the head ...
                let head = b.head.load(Ordering::Acquire);
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
    };
}

#[rustfmt::skip] // https://github.com/rust-lang/rustfmt/issues/5974
macro_rules! fn_producer_write_chunk {
    (N = $N:ident, bip = $bip:ident, contiguous = $contiguous:ident) => {
        /// Prepares a chunk of `n` slots (initially containing their [`Default`] value)
        /// for writing.
        ///
        #[doc = choice!($contiguous,
            /// [`WriteChunk::as_mut_slice()`]
            ::
            /// [`WriteChunk::as_mut_slices()`]
        )]
        /// provides mutable access to the slots.
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
        /// Use
        #[doc = choice!($bip,
            /// [`slots_contiguous_max()`](Producer::slots_contiguous_max) (or
            /// [`slots_contiguous_first()`](Producer::slots_contiguous_first))
            ::
            /// [`slots()`](Producer::slots)
        )]
        /// to obtain the number of available slots beforehand.
        ///
        /// # Examples
        ///
        /// See the documentation of the [`chunks`](crate::chunks#examples) module.
        pub fn write_chunk(
            &mut self,
            n: usize,
        ) -> Result<generic!(WriteChunk<'_>, N = $N), ChunkError>
        where
            T: Default,
        {
            self.write_chunk_uninit(n).map(WriteChunk::from)
        }
    };
}

macro_rules! fn_consumer_read_chunk_docstring {
    (bip = $bip:ident, contiguous = $contiguous:ident) => { docstring!(
        /// Prepares a chunk of `n` slots for reading.
        ///
        #[doc = choice!($contiguous,
            /// [`ReadChunk::as_slice()`]
            ::
            /// [`ReadChunk::as_slices()`]
        )]
        /// provides immutable access to the slots.
        /// After reading from those slots, they explicitly have to be made available
        /// to be written again by the [`Producer`] by calling [`ReadChunk::commit()`]
        /// or [`ReadChunk::commit_all()`].
        ///
        /// Alternatively, items can be moved out of the [`ReadChunk`] using iteration
        /// because it implements [`IntoIterator`]
        /// ([`ReadChunk::into_iter()`] can be used to explicitly turn it into an [`Iterator`]).
        /// All moved items are automatically made available to be written again by
        /// the [`Producer`].
        ///
        /// # Errors
        ///
        /// If not enough slots are available, an error
        /// (containing the number of available slots) is returned.
        /// Use
        #[doc = choice!($bip,
            /// [`slots_contiguous_first()`](Consumer::slots_contiguous_first)
            ::
            /// [`slots()`](Consumer::slots)
        )]
        /// to obtain the number of available slots beforehand.
        ///
        /// # Examples
        ///
        /// See the documentation of the [`chunks`](chunks#examples) module.
    )}
}

macro_rules! fn_consumer_read_chunk {
    (N = $N:ident, bip = yes, contiguous = yes) => {
        #[doc = fn_consumer_read_chunk_docstring!(bip = yes, contiguous = yes)]
        pub fn read_chunk(
            &mut self,
            n: usize,
        ) -> Result<generic!(ReadChunk<'_>, N = $N), ChunkError> {
            let b = &self.buffer;
            let (slots, _, _) = self.slots_contiguous_helper();
            if slots >= n {
                // TODO: use refreshed_head.or_else(self.cached_head.get)
                let offset = b.collapse_position(self.cached_head.get());
                // SAFETY: `offset` has been set to a valid position.
                Ok(unsafe { ReadChunk::new(self, n, offset) })
            } else {
                Err(ChunkError::TooFewSlots(slots))
            }
        }
    };
    (N = $N:ident, bip = no, contiguous = $contiguous:ident) => {
        #[doc = fn_consumer_read_chunk_docstring!(bip = no, contiguous = $contiguous)]
        pub fn read_chunk(
            &mut self,
            n: usize,
        ) -> Result<generic!(ReadChunk<'_>, N = $N), ChunkError> {
            let head = self.cached_head.get();
            let tail = self.cached_tail.get();
            let b = &self.buffer;
            // Check if the queue has *possibly* not enough slots.
            if b.distance(head, tail) < n {
                // Refresh the tail ...
                let tail = b.tail.load(Ordering::Acquire);
                self.cached_tail.set(tail);
                // ... and check if there *really* are not enough slots.
                let slots = b.distance(head, tail);
                if slots < n {
                    return Err(ChunkError::TooFewSlots(slots));
                }
            }
            let offset = b.collapse_position(head);
            // SAFETY: `offset` has been set to a valid position.
            Ok(unsafe { ReadChunk::new(self, n, offset) })
        }
    };
}

macro_rules! fn_write_chunk_uninit_as_mut_sliceX_docstring {
    () => { docstring!(
        /// After writing to the slots, they are *not* automatically made available
        /// to be read by the [`Consumer`].
        /// This has to be explicitly done by calling [`commit()`](WriteChunkUninit::commit)
        /// or [`commit_all()`](WriteChunkUninit::commit_all).
        /// If items are written but *not* committed afterwards,
        /// they will *not* become available for reading and
        /// they will be leaked (which is only relevant if `T` implements [`Drop`]).
    )};
}

macro_rules! fn_write_chunk_uninit_as_mut_sliceX {
    (contiguous = yes) => {
        /// Returns a slice for writing to the requested slots.
        ///
        /// The extension trait [`CopyToUninit`] can be used
        /// to safely copy data into this slice.
        ///
        #[doc = fn_write_chunk_uninit_as_mut_sliceX_docstring!()]
        pub fn as_mut_slice(&mut self) -> &mut [MaybeUninit<T>] {
            // SAFETY: The pointer and length have been computed correctly in write_chunk_uninit().
            unsafe { core::slice::from_raw_parts_mut(self.ptr.cast(), self.len) }
        }
    };
    (contiguous = no) => {
        /// Returns two slices for writing to the requested slots.
        ///
        /// The first slice can only be empty if `0` slots have been requested.
        /// If the first slice contains all requested slots, the second one is empty.
        ///
        /// The extension trait [`CopyToUninit`] can be used
        /// to safely copy data into those slices.
        ///
        #[doc = fn_write_chunk_uninit_as_mut_sliceX_docstring!()]
        pub fn as_mut_slices(&mut self) -> (&mut [MaybeUninit<T>], &mut [MaybeUninit<T>]) {
            // SAFETY: The pointers and lengths have been computed correctly in write_chunk_uninit().
            unsafe {
                (
                    core::slice::from_raw_parts_mut(self.first_ptr.cast(), self.first_len),
                    core::slice::from_raw_parts_mut(self.second_ptr.cast(), self.second_len),
                )
            }
        }
    };
}

/// Drops all elements starting from index `n`.
///
/// # Safety
///
/// All of those slots must be initialized.
macro_rules! fn_write_chunk_uninit_drop_suffix {
    (contiguous = yes) => {
        unsafe fn drop_suffix(&mut self, n: usize) {
            // NB: If n >= self.len(), the loop is not entered.
            for i in n..self.len {
                // SAFETY: The caller must make sure that all slots are initialized.
                unsafe { self.ptr.add(i).drop_in_place() };
            }
        }
    };
    (contiguous = no) => {
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
    };
}

macro_rules! fn_write_chunk_as_mut_sliceX_docstring {
    () => { docstring!(
        /// After writing to the slots, they are *not* automatically made available
        /// to be read by the [`Consumer`].
        /// This has to be explicitly done by calling [`commit()`](WriteChunk::commit)
        /// or [`commit_all()`](WriteChunk::commit_all).
        /// If items are written but *not* committed afterwards,
        /// they will *not* become available for reading and
        /// they will eventually be dropped (if `T` implements [`Drop`]).
    )};
}

macro_rules! fn_write_chunk_as_mut_sliceX {
    (contiguous = yes) => {
        /// Returns a slice for writing to the requested slots.
        ///
        /// All slots are initially filled with their [`Default`] value.
        ///
        #[doc = fn_write_chunk_as_mut_sliceX_docstring!()]
        pub fn as_mut_slice(&mut self) -> &mut [T] {
            // self.0 is always Some(chunk).
            let chunk = self.0.as_ref().unwrap();
            // SAFETY: The pointer and length have been computed correctly in write_chunk_uninit()
            // and all slots have been initialized in From::from().
            unsafe { core::slice::from_raw_parts_mut(chunk.ptr, chunk.len) }
        }
    };
    (contiguous = no) => {
        /// Returns two slices for writing to the requested slots.
        ///
        /// All slots are initially filled with their [`Default`] value.
        ///
        /// The first slice can only be empty if `0` slots have been requested.
        /// If the first slice contains all requested slots, the second one is empty.
        ///
        #[doc = fn_write_chunk_as_mut_sliceX_docstring!()]
        pub fn as_mut_slices(&mut self) -> (&mut [T], &mut [T]) {
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
    };
}

macro_rules! fn_read_chunk_as_sliceX_docstring {
    () => { docstring!(
        /// The provided slots are *not* automatically made available
        /// to be written again by the [`Producer`].
        /// This has to be explicitly done by calling [`commit()`](ReadChunk::commit)
        /// or [`commit_all()`](ReadChunk::commit_all).
        /// Note that this runs the destructor of the committed items (if `T` implements [`Drop`]).
        /// You can "peek" at the contained values by simply not calling any of the "commit" methods.
    )};
}

macro_rules! fn_read_chunk_as_sliceX {
    (contiguous = yes) => {
        /// Returns a slice for reading from the requested slots.
        ///
        #[doc = fn_read_chunk_as_sliceX_docstring!()]
        pub fn as_slice(&self) -> &[T] {
            // SAFETY: The correct pointer and length have been provided by ReadChunk::new().
            unsafe { core::slice::from_raw_parts(self.ptr, self.len) }
        }
    };
    (contiguous = no) => {
        /// Returns two slices for reading from the requested slots.
        ///
        /// The first slice can only be empty if `0` slots have been requested.
        /// If the first slice contains all requested slots, the second one is empty.
        ///
        #[doc = fn_read_chunk_as_sliceX_docstring!()]
        pub fn as_slices(&self) -> (&[T], &[T]) {
            // SAFETY: The pointers and lengths have been computed correctly in read_chunk().
            unsafe {
                (
                    core::slice::from_raw_parts(self.first_ptr, self.first_len),
                    core::slice::from_raw_parts(self.second_ptr, self.second_len),
                )
            }
        }
    };
}

macro_rules! fn_read_chunk_as_mut_sliceX_docstring {
    () => { docstring!(
        /// In the vast majority of cases, mutable access is not required when
        /// reading data and the immutable version should be preferred. However,
        /// there are some scenarios where it might be desirable to perform
        /// operations on the data in-place without copying it to a separate buffer
        /// (e.g. streaming decryption), in which case this version can be used.
    )};
}

macro_rules! fn_read_chunk_as_mut_sliceX {
    (contiguous = yes) => {
        /// Returns a mutable slice for reading from the requested slots.
        ///
        /// This has the same semantics as [`as_slice()`](ReadChunk::as_slice),
        /// except that it returns a mutable slice and requires a mutable reference
        /// to the chunk.
        ///
        #[doc = fn_read_chunk_as_mut_sliceX_docstring!()]
        pub fn as_mut_slice(&mut self) -> &mut [T] {
            // SAFETY: The correct pointer and length have been provided by ReadChunk::new().
            unsafe { core::slice::from_raw_parts_mut(self.ptr, self.len) }
        }
    };
    (contiguous = no) => {
        /// Returns two mutable slices for reading from the requested slots.
        ///
        /// This has the same semantics as [`as_slices()`](ReadChunk::as_slices),
        /// except that it returns mutable slices and requires a mutable reference
        /// to the chunk.
        ///
        #[doc = fn_read_chunk_as_mut_sliceX_docstring!()]
        pub fn as_mut_slices(&mut self) -> (&mut [T], &mut [T]) {
            // SAFETY: The pointers and lengths have been computed correctly in read_chunk().
            unsafe {
                (
                    core::slice::from_raw_parts_mut(self.first_ptr, self.first_len),
                    core::slice::from_raw_parts_mut(self.second_ptr, self.second_len),
                )
            }
        }
    };
}

macro_rules! fn_X_chunk_X_len_and_is_empty {
    (contiguous = yes) => {
        /// Returns the number of slots in the chunk.
        pub fn len(&self) -> usize {
            self.len
        }

        /// Returns `true` if the chunk contains no slots.
        pub fn is_empty(&self) -> bool {
            self.len == 0
        }
    };
    (contiguous = no) => {
        /// Returns the number of slots in the chunk.
        pub fn len(&self) -> usize {
            self.first_len + self.second_len
        }

        /// Returns `true` if the chunk contains no slots.
        pub fn is_empty(&self) -> bool {
            self.first_len == 0
        }
    };
}

macro_rules! fn_write_chunk_len_and_is_empty {
    () => {
        /// Returns the number of slots in the chunk.
        pub fn len(&self) -> usize {
            // self.0 is always Some(chunk).
            self.0.as_ref().unwrap().len()
        }

        /// Returns `true` if the chunk contains no slots.
        pub fn is_empty(&self) -> bool {
            // self.0 is always Some(chunk).
            self.0.as_ref().unwrap().is_empty()
        }
    };
}

macro_rules! struct_write_chunk_uninit {
    (N = $N:ident, arc = $arc:ident, contiguous = yes) => {
        // TODO: implement manually:
        //#[derive(Debug, PartialEq, Eq)]
        struct_!(
            /// Structure for writing into multiple (uninitialized) slots in one go.
            ///
            /// This is returned from [`Producer::write_chunk_uninit()`].
            pub WriteChunkUninit<'a>, N = $N, {
                ptr: *mut T,
                len: usize,
                producer: &'a generic!(Producer, N = $N, arc = $arc),
            }
        );

        impl_!(WriteChunkUninit<'a>, N = $N, {
            pub(super) unsafe fn new(producer: &'a generic!(Producer, N = $N, arc = $arc), n: usize, offset: usize) -> Self {
                Self {
                    // SAFETY: Caller must guarantee that `offset` is valid.
                    ptr: unsafe { producer.buffer.data_ptr().add(offset) },
                    len: n,
                    producer,
                }
            }
        });

        impl_!(WriteChunk<'a>, trait = From<generic!(WriteChunkUninit<'a>, N = $N)>, N = $N,
        where
            T: Default,
        {
            /// Fills all slots with the [`Default`] value.
            fn from(chunk: generic!(WriteChunkUninit<'a>, N = $N)) -> Self {
                for i in 0..chunk.len {
                    // SAFETY: i is in a valid range.
                    unsafe { chunk.ptr.add(i).write(Default::default()) };
                }
                WriteChunk(Some(chunk))
            }
        });
    };
    (N = $N:ident, arc = $arc:ident, contiguous = no) => {
        // TODO: implement manually:
        //#[derive(Debug, PartialEq, Eq)]
        struct_!(
            /// Structure for writing into multiple (uninitialized) slots in one go.
            ///
            /// This is returned from [`Producer::write_chunk_uninit()`].
            pub WriteChunkUninit<'a>, N = $N, {
                first_ptr: *mut T,
                first_len: usize,
                second_ptr: *mut T,
                second_len: usize,
                producer: &'a generic!(Producer, N = $N, arc = $arc),
            }
        );

        impl_!(WriteChunkUninit<'a>, N = $N, {
            pub(super) unsafe fn new(producer: &'a generic!(Producer, N = $N, arc = $arc), n: usize, offset: usize) -> Self {
                let first_len = n.min(producer.buffer.capacity() - offset);
                Self {
                    // SAFETY: Caller must guarantee that `offset` is valid.
                    first_ptr: unsafe { producer.buffer.data_ptr().add(offset) },
                    first_len,
                    second_ptr: producer.buffer.data_ptr(),
                    second_len: n - first_len,
                    producer,
                }
            }
        });

        impl_!(WriteChunk<'a>, trait = From<generic!(WriteChunkUninit<'a>, N = $N)>, N = $N,
        where
            T: Default,
        {
            /// Fills all slots with the [`Default`] value.
            fn from(chunk: generic!(WriteChunkUninit<'a>, N = $N)) -> Self {
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
        });
    };
}

macro_rules! struct_write_chunk {
    (N = $N:ident) => {
        //#[derive(Debug, PartialEq, Eq)]
        struct_!(
            /// Structure for writing into multiple ([`Default`]-initialized) slots in one go.
            ///
            /// This is returned from [`Producer::write_chunk()`].
            ///
            /// To obtain uninitialized slots, use [`Producer::write_chunk_uninit()`] instead,
            /// which also allows moving items from an iterator into the ring buffer
            /// by means of [`WriteChunkUninit::fill_from_iter()`].
            pub WriteChunk<'a>, N = $N, (Option<generic!(WriteChunkUninit<'a>, N = $N)>););

        impl_!(WriteChunk<'_>, trait = Drop, N = $N, {
            fn drop(&mut self) {
                // NB: If `commit()` or `commit_all()` has been called, `self.0` is `None`.
                if let Some(mut chunk) = self.0.take() {
                    // No part of the chunk has been committed, all slots are dropped.
                    // SAFETY: All slots have been initialized in From::from().
                    unsafe { chunk.drop_suffix(0) };
                }
            }
        });
    };
}

macro_rules! struct_read_chunk {
    (N = $N:ident, arc = $arc:ident, contiguous = yes) => {
        // TODO: implement manually:
        //#[derive(Debug, PartialEq, Eq)]
        struct_!(
            /// Structure for reading from multiple slots in one go.
            ///
            /// This is returned from [`Consumer::read_chunk()`].
            pub ReadChunk<'a>, N = $N, {
                ptr: *mut T,
                len: usize,
                consumer: &'a generic!(Consumer, N = $N, arc = $arc),
            }
        );

        impl_!(ReadChunk<'a>, N = $N, {
            pub(super) unsafe fn new(consumer: &'a generic!(Consumer, N = $N, arc = $arc), n: usize, offset: usize) -> Self {
                Self {
                    // SAFETY: Caller must guarantee that `offset` is valid.
                    ptr: unsafe { consumer.buffer.data_ptr().add(offset) },
                    len: n,
                    consumer,
                }
            }
        });
    };
    (N = $N:ident, arc = $arc:ident, contiguous = no) => {
        // TODO: implement manually:
        //#[derive(Debug, PartialEq, Eq)]
        struct_!(
            /// Structure for reading from multiple slots in one go.
            ///
            /// This is returned from [`Consumer::read_chunk()`].
            pub ReadChunk<'a>, N = $N, {
                first_ptr: *mut T,
                first_len: usize,
                second_ptr: *mut T,
                second_len: usize,
                consumer: &'a generic!(Consumer, N = $N, arc = $arc),
            }
        );

        impl_!(ReadChunk<'a>, N = $N, {
            pub(super) unsafe fn new(consumer: &'a generic!(Consumer, N = $N, arc = $arc), n: usize, offset: usize) -> Self {
                let b = &consumer.buffer;
                let first_len = n.min(b.capacity() - offset);
                Self {
                    // SAFETY: Caller must guarantee that `offset` is valid.
                    first_ptr: unsafe { b.data_ptr().add(offset) },
                    first_len,
                    second_ptr: b.data_ptr(),
                    second_len: n - first_len,
                    consumer,
                }
            }
        });
    };
}

macro_rules! fn_write_chunk_uninit_commit_all {
    () => {
        /// Makes the whole chunk available for reading.
        ///
        /// # Safety
        ///
        /// The caller must make sure that all elements have been initialized.
        pub unsafe fn commit_all(self) {
            let slots = self.len();
            // SAFETY: Delegated to the caller.
            unsafe { self.commit_unchecked(slots) };
        }
    };
}

macro_rules! fn_write_chunk_uninit_commit {
    () => {
        /// Makes the first `n` slots of the chunk available for reading.
        ///
        /// # Panics
        ///
        /// Panics if `n` is greater than the number of slots in the chunk.
        ///
        /// # Safety
        ///
        /// The caller must make sure that the first `n` elements have been initialized.
        pub unsafe fn commit(self, n: usize) {
            assert!(n <= self.len(), "cannot commit more than chunk size");
            // SAFETY: Delegated to the caller.
            unsafe { self.commit_unchecked(n) };
        }
    };
}

macro_rules! fn_write_chunk_uninit_fill_from_iter_docstring {
    (N = $N:ident, arc = $arc:ident, module = $module:literal) => { docstring!(
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
        #[doc = doctest_import!($module, "{PopError, RingBuffer}")]
        ///
        #[doc = doctest_create_ring_buffer!(N = $N, arc = $arc, capacity = 4)]
        /// if let Ok(chunk) = p.write_chunk_uninit(3) {
        ///     assert_eq!(chunk.fill_from_iter([10, 20]), 2);
        /// } else {
        ///     unreachable!();
        /// }
        /// assert_eq!(c.slots(), 2);
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
        #[doc = doctest_import!($module, "{PopError, RingBuffer}")]
        ///
        #[doc = doctest_create_ring_buffer!(N = $N, arc = $arc, capacity = 4)]
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
    )}
}

macro_rules! fn_write_chunk_uninit_fill_from_iter {
    (N = $N:ident, arc = $arc:ident, contiguous = yes, module = $module:literal) => {
        #[doc = fn_write_chunk_uninit_fill_from_iter_docstring!(N = $N, arc = $arc, module = $module)]
        pub fn fill_from_iter<I>(self, iter: I) -> usize
        where
            I: IntoIterator<Item = T>,
        {
            let mut iter = iter.into_iter();
            let mut iterated = 0;
            for i in 0..self.len {
                match iter.next() {
                    Some(item) => {
                        // SAFETY: It is allowed to write to this memory slot
                        unsafe { self.ptr.add(i).write(item) };
                        iterated += 1;
                    }
                    None => break,
                }
            }
            // SAFETY: iterated slots have been initialized above
            unsafe { self.commit_unchecked(iterated) }
        }
    };
    (N = $N:ident, arc = $arc:ident, contiguous = no, module = $module:literal) => {
        #[doc = fn_write_chunk_uninit_fill_from_iter_docstring!(N = $N, arc = $arc, module = $module)]
        pub fn fill_from_iter<I>(self, iter: I) -> usize
        where
            I: IntoIterator<Item = T>,
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
    };
}
macro_rules! fn_write_chunk_commit_all {
    () => {
        /// Makes the whole chunk available for reading.
        pub fn commit_all(mut self) {
            // self.0 is always Some(chunk).
            let chunk = self.0.take().unwrap();
            // SAFETY: All slots have been initialized in From::from().
            unsafe { chunk.commit_all() };
            // `self` is dropped here, with `self.0` being set to `None`.
        }
    };
}

macro_rules! fn_write_chunk_commit {
    () => {
        /// Makes the first `n` slots of the chunk available for reading.
        ///
        /// The rest of the chunk is dropped.
        ///
        /// # Panics
        ///
        /// Panics if `n` is greater than the number of slots in the chunk.
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
    };
}

macro_rules! fn_read_chunk_commit_all {
    () => {
        /// Drops all slots of the chunk, making the space available for writing again.
        pub fn commit_all(self) {
            let slots = self.len();
            // SAFETY: self.len() initialized elements have been obtained in read_chunk().
            unsafe { self.commit_unchecked(slots) };
        }
    };
}

macro_rules! fn_read_chunk_commit {
    (N = $N:ident, arc = $arc:ident, module = $module:literal) => {
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
        #[doc = doctest_import!($module, "RingBuffer")]
        ///
        /// // Static variable to count all drop() invocations
        /// static mut DROP_COUNT: i32 = 0;
        /// #[derive(Debug)]
        /// struct Thing(u8);
        /// impl Drop for Thing {
        ///     fn drop(&mut self) { unsafe { DROP_COUNT += 1; } }
        /// }
        ///
        /// // Scope to limit lifetime of ring buffer
        /// {
        #[doc = doctest_create_ring_buffer!(N = $N, arc = $arc, capacity = 4, "    ")]
        ///
        ///     assert!(p.push(Thing(1)).is_ok());
        ///     assert!(p.push(Thing(2)).is_ok());
        ///     if let Ok(thing) = c.pop() {
        ///         // "thing" has been *moved* out of the queue but not yet dropped
        ///         assert_eq!(unsafe { DROP_COUNT }, 0);
        ///     } else {
        ///         unreachable!();
        ///     }
        ///     // First Thing has been dropped when "thing" went out of scope:
        ///     assert_eq!(unsafe { DROP_COUNT }, 1);
        ///     assert!(p.push(Thing(3)).is_ok());
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
        pub fn commit(self, n: usize) {
            assert!(n <= self.len(), "cannot commit more than chunk size");
            // SAFETY: self.len() initialized elements have been obtained in read_chunk().
            unsafe { self.commit_unchecked(n) };
        }
    };
}

macro_rules! impl_send_for_chunks {
    (N = $N:ident, module = $module:literal) => {
        // SAFETY: WriteChunkUninit only exists while a unique reference to the producer is held.
        // It is therefore safe to move it to another thread.
        impl_!(
            /// It (as well as [`WriteChunk`]) can be moved ...
            /// ```
            #[doc = doctest_import!($module, "chunks::{WriteChunk, WriteChunkUninit}")]
            /// fn assert_send<X: Send>() {}
            #[doc = concat!("assert_send::<", doctest_ty!(WriteChunkUninit, u8, 8, N = $N), ">();")]
            #[doc = concat!("assert_send::<", doctest_ty!(WriteChunk, u8, 8, N = $N), ">();")]
            /// ```
            /// ... but not shared between threads:
            /// ```compile_fail
            #[doc = doctest_import!($module, "chunks::WriteChunkUninit", "# ")]
            /// fn assert_sync<X: Sync>() {}
            #[doc = concat!("assert_sync::<", doctest_ty!(WriteChunkUninit, u8, 8, N = $N), ">();")]
            /// ```
            /// ```compile_fail
            #[doc = doctest_import!($module, "chunks::WriteChunk", "# ")]
            /// # fn assert_sync<X: Sync>() {}
            #[doc = concat!("assert_sync::<", doctest_ty!(WriteChunk, u8, 8, N = $N), ">();")]
            /// ```
            WriteChunkUninit<'_>, trait unsafe = Send, N = $N, where T: Send {});

        // SAFETY: ReadChunk only exists while a unique reference to the consumer is held.
        // It is therefore safe to move it to another thread.
        impl_!(
            /// It (and any wrapper structs) can be moved ...
            /// ```
            #[doc = doctest_import!($module, "chunks::ReadChunk")]
            /// fn assert_send<X: Send>() {}
            #[doc = concat!("assert_send::<", doctest_ty!(ReadChunk, u8, 8, N = $N), ">();")]
            /// ```
            /// ... but not shared between threads:
            /// ```compile_fail
            #[doc = doctest_import!($module, "chunks::ReadChunk", "# ")]
            /// fn assert_sync<X: Sync>() {}
            #[doc = concat!("assert_sync::<", doctest_ty!(ReadChunk, u8, 8, N = $N), ">();")]
            /// ```
            ReadChunk<'_>, trait unsafe = Send, N = $N, where T: Send {});
    };
}

macro_rules! impl_debug {
    (N = $N:ident, arc = $arc:ident, $($chunk:ident),*) => {
        $(
            impl_!($chunk, trait = fmt::Debug, N = $N, arc = $arc, {
                fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                    write!(f, stringify!($chunk))
                }
            });
        )*
    };
    (N = $N:ident, $($chunk:ident$(<$lifetime:lifetime>)?),*) => {
        $(
            impl_!($chunk$(<$lifetime>)?, trait = fmt::Debug, N = $N, {
                fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                    write!(f, stringify!($chunk))
                }
            });
        )*
    };
}

macro_rules! impl_into_iterator_for_read_chunk {
    (N = $N:ident, contiguous = $contiguous:ident) => {
        impl_!(ReadChunk<'a>, trait = IntoIterator, N = $N, {
            type Item = T;
            type IntoIter = generic!(ReadChunkIntoIter<'a>, N = $N);

            /// Turns a [`ReadChunk`] into an iterator.
            ///
            /// When the iterator is dropped, all iterated slots are made available for writing again.
            /// Non-iterated items remain in the ring buffer.
            fn into_iter(self) -> Self::IntoIter {
                Self::IntoIter {
                    chunk: self,
                    iterated: 0,
                }
            }
        });

        struct_!(
            /// An iterator that moves out of a [`ReadChunk`].
            ///
            /// This `struct` is created by the [`into_iter()`](ReadChunk::into_iter) method
            /// on [`ReadChunk`] (provided by the [`IntoIterator`] trait).
            ///
            /// When this `struct` is dropped, the iterated slots are made available for writing again.
            /// Non-iterated items remain in the ring buffer.
            pub ReadChunkIntoIter<'a>, N = $N, {
                chunk: generic!(ReadChunk<'a>, N = $N),
                iterated: usize,
            }
        );

        // TODO: take "skip" into account?
        impl_!(ReadChunkIntoIter<'_>, trait = Drop, N = $N, {
            /// Makes all iterated slots available for writing again.
            ///
            /// All iterated items have been moved out of the buffer and
            /// don't need to be dropped here.
            ///
            /// Non-iterated items remain in the ring buffer and are *not* dropped.
            fn drop(&mut self) {
                let c = self.chunk.consumer;
                let head = c.buffer.increment(c.cached_head.get(), self.iterated);
                c.buffer.head.store(head, Ordering::Release);
                c.cached_head.set(head);
            }
        });

        impl_!(ReadChunkIntoIter<'_>, trait = Iterator, N = $N, {
            type Item = T;

            fn_read_chunk_into_iter_next!(contiguous = $contiguous);
            fn_read_chunk_into_iter_size_hint!(contiguous = $contiguous);
        });

        impl_!(ReadChunkIntoIter<'_>, trait = ExactSizeIterator, N = $N, {});

        impl_!(ReadChunkIntoIter<'_>, trait = core::iter::FusedIterator, N = $N, {});
    };
}

macro_rules! fn_read_chunk_into_iter_next {
    (contiguous = yes) => {
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
    };
    (contiguous = no) => {
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
    };
}

macro_rules! fn_read_chunk_into_iter_size_hint {
    (contiguous = yes) => {
        fn size_hint(&self) -> (usize, Option<usize>) {
            let remaining = self.chunk.len - self.iterated;
            (remaining, Some(remaining))
        }
    };
    (contiguous = no) => {
        fn size_hint(&self) -> (usize, Option<usize>) {
            let remaining = self.chunk.first_len + self.chunk.second_len - self.iterated;
            (remaining, Some(remaining))
        }
    };
}

macro_rules! mod_chunks_docstring {
    (storage = $storage:ident, N = $N:ident, arc = $arc:ident, bip = $bip:ident, contiguous = $contiguous:ident, module = $module:literal) => { docstring!(
/// Writing and reading multiple items at once into and from a [`RingBuffer`].
///
/// Multiple items at once can be moved from an iterator into the ring buffer by using
/// [`Producer::write_chunk_uninit()`] followed by [`WriteChunkUninit::fill_from_iter()`].
/// Alternatively, mutable access to the (uninitialized) slots of the chunk can be obtained with
#[doc = choice!($contiguous,
    /// [`WriteChunkUninit::as_mut_slice()`],
    ::
    /// [`WriteChunkUninit::as_mut_slices()`],
)]
/// which requires writing some `unsafe` code.
/// To avoid that, [`Producer::write_chunk()`] can be used,
/// which initializes all slots with their [`Default`] value
/// and provides mutable access by means of
#[doc = choice!($contiguous,
    /// [`WriteChunk::as_mut_slice()`].
    ::
    /// [`WriteChunk::as_mut_slices()`].
)]
///
/// Multiple items at once can be moved out of the ring buffer by using
/// [`Consumer::read_chunk()`] and iterating over the returned [`ReadChunk`]
/// (or by explicitly calling [`ReadChunk::into_iter()`]).
/// Immutable access to the slots of the chunk can be obtained with
#[doc = choice!($contiguous,
    /// [`ReadChunk::as_slice()`].
    ::
    /// [`ReadChunk::as_slices()`].
)]
#[doc = docstring_exclude_vrb!($storage,
    ///
    /// # Examples
    ///
    /// This example uses a single thread for simplicity, but in a real application,
    /// `p` and `c` would of course live on different threads:
    ///
    /// ```
    #[doc = doctest_import!($module, "RingBuffer")]
    ///
    #[doc = doctest_create_ring_buffer!(N = $N, arc = $arc, capacity = 4)]
    ///
    /// if let Ok(chunk) = p.write_chunk_uninit(4) {
    ///     chunk.fill_from_iter([10, 11, 12]);
    ///     // Note that we requested 4 slots but we've only written to 3 of them!
    /// } else {
    ///     unreachable!();
    /// }
    ///
    /// assert_eq!(p.slots(), 1);
    /// assert_eq!(c.slots(), 3);
    #[doc = choice!($bip,
        /// // Note that all of those slots are available in a single contiguous chunk:
        /// assert_eq!(c.slots_contiguous(), (3, 0));
        ::
    )]
    ///
    /// if let Ok(chunk) = c.read_chunk(2) {
    ///     assert_eq!(chunk.into_iter().collect::<Vec<_>>(), [10, 11]);
    /// } else {
    ///     unreachable!();
    /// }
    ///
    /// // One element is still in the queue:
    /// assert_eq!(c.peek(), Ok(&12));
    ///
    /// assert_eq!(p.slots(), 3);
    #[doc = choice!($bip,
        /// // Those free slots are available in two contiguous chunks:
        /// assert_eq!(p.slots_contiguous(), (1, 2));
        ::
    )]
    ///
    /// let data = vec![20, 21];
    /// // NB: write_chunk_uninit() could be used for possibly better performance:
    /// if let Ok(mut chunk) = p.write_chunk(2) {
    #[doc = choice!($bip,
        ///     // A chunk of 2 was not available at the end of the buffer,
        ///     // so one slot was skipped and we got a chunk at the beginning.
        ///     chunk.as_mut_slice().copy_from_slice(&data);
        ::
        ///     let (first, second) = chunk.as_mut_slices();
        ///     let mid = first.len();
        ///     first.copy_from_slice(&data[..mid]);
        ///     second.copy_from_slice(&data[mid..]);
    )]
    ///     chunk.commit_all();
    /// } else {
    ///     unreachable!();
    /// }
    ///
    /// assert_eq!(c.slots(), 3);
    #[doc = choice!($bip,
        /// assert_eq!(c.slots_contiguous(), (1, 2));
        /// assert!(p.is_full()); // The skipped slot is not available for writing (for now)!
        /// assert_eq!(c.pop(), Ok(12));
        /// // Popping this element has *not* reset the read index,
        /// // making only one of two empty slots available for writing:
        /// assert_eq!(p.slots(), 1);
        /// // Any operation on the consumer (except capacity() and is_abandoned()) ...
        /// assert_eq!(c.peek(), Ok(&20));
        /// // ... will reset the read index and make both slots available for writing:
        /// assert_eq!(p.slots(), 2);
        ///
        /// let mut v = Vec::<i32>::with_capacity(2);
        /// if let Ok(chunk) = c.read_chunk(2) {
        ///     v.extend(chunk.as_slice());
        ///     chunk.commit_all();
        /// } else {
        ///     unreachable!();
        /// }
        ///
        /// assert_eq!(v, [20, 21]);
        ::
        /// assert_eq!(p.slots(), 1);
        ///
        /// let mut v = Vec::<i32>::with_capacity(3);
        /// if let Ok(chunk) = c.read_chunk(3) {
        ///     let (first, second) = chunk.as_slices();
        ///     v.extend(first);
        ///     v.extend(second);
        ///     chunk.commit_all();
        /// } else {
        ///     unreachable!();
        /// }
        ///
        /// assert_eq!(v, [12, 20, 21]);
    )]
    /// assert!(c.is_empty());
    /// ```
    ///
    #[doc = choice!($bip,
        /// TODO: modify iterator example, use slots_contiguous()
        ::
        /// The iterator API can be used to move items from one ring buffer to another:
        ///
        /// ```
        #[doc = doctest_import!($module, "{Consumer, Producer}")]
        ///
        /// // TODO: make this work for "array" variants:
        /// //fn move_items<T>(src: &mut Consumer<T>, dst: &mut Producer<T>) -> usize {
        /// //    let n = src.slots().min(dst.slots());
        /// //    dst.write_chunk_uninit(n).unwrap().fill_from_iter(src.read_chunk(n).unwrap())
        /// //}
        /// ```
    )]
)]
///
/// ## Common Access Patterns
///
/// TODO: does this make sense for "contiguous" variants?
///
/// The following examples show the [`Producer`] side;
/// similar patterns can of course be used with [`Consumer::read_chunk()`] as well.
/// Furthermore, the examples use [`Producer::write_chunk_uninit()`],
/// along with a bit of `unsafe` code.
/// To avoid this, you can use [`Producer::write_chunk()`] instead,
/// which requires the trait bound `T: Default` and will lead to a small runtime overhead.
///
/// Copy a whole slice of items into the ring buffer, but only if space permits
/// (if not, the entire input slice is returned as an error):
///
/// ```
/// use rtrb::{Producer, CopyToUninit as _};
/// // TODO:
#[doc = doctest_import!($module, "{Producer, CopyToUninit as _}", "// ")]
///
/// fn push_entire_slice<'a, T>(queue: &mut Producer<T>, slice: &'a [T]) -> Result<(), &'a [T]>
/// where
///     T: Copy,
/// {
///     if let Ok(mut chunk) = queue.write_chunk_uninit(slice.len()) {
///         let (first, second) = chunk.as_mut_slices();
///         let mid = first.len();
///         slice[..mid].copy_to_uninit(first);
///         slice[mid..].copy_to_uninit(second);
///         // SAFETY: All slots have been initialized
///         unsafe { chunk.commit_all() };
///         Ok(())
///     } else {
///         Err(slice)
///     }
/// }
/// ```
///
/// Copy as many items as possible from a given slice, returning the number of copied items:
///
/// ```
/// use rtrb::{Producer, CopyToUninit as _, ChunkError::TooFewSlots};
/// // TODO:
#[doc = doctest_import!($module, "{Producer, CopyToUninit as _, ChunkError::TooFewSlots}", "// ")]
///
/// fn push_partial_slice<T>(queue: &mut Producer<T>, slice: &[T]) -> usize
/// where
///     T: Copy,
/// {
///     let mut chunk = match queue.write_chunk_uninit(slice.len()) {
///         Ok(chunk) => chunk,
///         // Remaining slots are returned, this will always succeed:
///         Err(TooFewSlots(n)) => queue.write_chunk_uninit(n).unwrap(),
///     };
///     let end = chunk.len();
///     let (first, second) = chunk.as_mut_slices();
///     let mid = first.len();
///     slice[..mid].copy_to_uninit(first);
///     slice[mid..end].copy_to_uninit(second);
///     // SAFETY: All slots have been initialized
///     unsafe { chunk.commit_all() };
///     end
/// }
/// ```
///
/// Write as many slots as possible, given an iterator
/// (and return the number of written slots):
///
/// ```
/// use rtrb::{Producer, ChunkError::TooFewSlots};
/// // TODO:
#[doc = doctest_import!($module, "{Producer, ChunkError::TooFewSlots}", "// ")]
///
/// fn push_from_iter<T, I>(queue: &mut Producer<T>, iter: I) -> usize
/// where
///     T: Default,
///     I: IntoIterator<Item = T>,
/// {
///     let iter = iter.into_iter();
///     let n = match iter.size_hint() {
///         (_, None) => queue.slots(),
///         (_, Some(n)) => n,
///     };
///     let chunk = match queue.write_chunk_uninit(n) {
///         Ok(chunk) => chunk,
///         // Remaining slots are returned, this will always succeed:
///         Err(TooFewSlots(n)) => queue.write_chunk_uninit(n).unwrap(),
///     };
///     chunk.fill_from_iter(iter)
/// }
/// ```
    )};
}
