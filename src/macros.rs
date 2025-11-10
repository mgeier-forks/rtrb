macro_rules! storage {
    (storage = array, arc = $arc:ident, bip = yes, padded = $padded:ident, rb_doc = $rb_doc:expr) => {
        storage_array!(arc = $arc, padded = $padded, skip, rb_doc = $rb_doc);
    };
    (storage = array, arc = $arc:ident, bip = no, padded = $padded:ident, rb_doc = $rb_doc:expr) => {
        storage_array!(arc = $arc, padded = $padded, , rb_doc = $rb_doc);
    };
    (storage = dst, arc = $arc:ident, bip = yes, padded = $padded:ident, rb_doc = $rb_doc:expr) => {
        compile_error!("TODO: implement");
    };
    (storage = dst, arc = $arc:ident, bip = no, padded = $padded:ident, rb_doc = $rb_doc:expr) => {
        compile_error!("TODO: implement");
    };
    (storage = vec, arc = $arc:ident, bip = yes, padded = $padded:ident, rb_doc = $rb_doc:expr) => {
        storage_vec!(padded = $padded, skip, rb_doc = $rb_doc);
    };
    (storage = vec, arc = $arc:ident, bip = no, padded = $padded:ident, rb_doc = $rb_doc:expr) => {
        storage_vec!(padded = $padded, , rb_doc = $rb_doc);
    };
    (storage = vrb, arc = $arc:ident, bip = $bip:ident, padded = $padded:ident, rb_doc = $rb_doc:expr) => {
        storage_vrb!(padded = $padded, rb_doc = $rb_doc);
    };
}

#[cfg(feature = "alloc")]
macro_rules! storage_vec {
    (padded = $padded:ident, $($skip:ident)?, rb_doc = $rb_doc:expr) => {
        use alloc::vec::Vec;
        use core::mem::ManuallyDrop;

        #[doc = $rb_doc]
        // TODO: manually derive Debug
        //#[derive(Debug)]
        pub struct RingBuffer<T> {
            head: choice_ty!($padded, CachePadded<AtomicUsize>, AtomicUsize),
            tail: choice_ty!($padded, CachePadded<AtomicUsize>, AtomicUsize),
            $(
                // TODO: measure whether CachePadded helps
                $skip: choice_ty!($padded, CachePadded<AtomicUsize>, AtomicUsize),
            )?
            flags: AtomicU8,
            /// The buffer holding slots.
            data_ptr: *mut T,
            capacity: usize,
        }

        // TODO: move to common implementation?
        // SAFETY: If T can be moved between threads, RingBuffer can as well.
        unsafe impl<T: Send> Send for RingBuffer<T> {}

        impl<T> RingBuffer<T> {
            // Private helper function.
            fn construct(capacity: usize) -> Self {
                Self {
                    head: choice!($padded,
                        CachePadded::new(AtomicUsize::new(0)),
                        AtomicUsize::new(0)),
                    tail: choice!($padded,
                        CachePadded::new(AtomicUsize::new(0)),
                        AtomicUsize::new(0)),
                    $(
                        $skip: choice!($padded,
                            CachePadded::new(AtomicUsize::new(capacity)),
                            AtomicUsize::new(capacity)),
                    )?
                    flags: AtomicU8::new(0),
                    data_ptr: ManuallyDrop::new(Vec::with_capacity(capacity)).as_mut_ptr(),
                    capacity,
                }
            }

            fn capacity(&self) -> usize {
                self.capacity
            }

            fn data_ptr(&self) -> *mut T {
                self.data_ptr
            }
        }

        impl<T> Drop for RingBuffer<T> {
            /// Drops all non-empty slots and deallocates the storage.
            fn drop(&mut self) {
                // SAFETY: this is called exactly once, no references to any elements exist anymore.
                unsafe { self.drop_all_elements() };

                // Finally, deallocate the buffer, but don't run any destructors.
                // SAFETY: data_ptr and capacity are still valid from the original initialization.
                unsafe { Vec::from_raw_parts(self.data_ptr, 0, self.capacity()) };
            }
        }
    };
}

macro_rules! storage_array {
    (arc = $arc:ident, padded = $padded:ident, $($skip:ident)?, rb_doc = $rb_doc:expr) => {
        use core::cell::UnsafeCell;
        use core::mem::MaybeUninit;

        #[doc = $rb_doc]
        // TODO: manually derive Debug
        //#[derive(Debug)]
        pub struct RingBuffer<T, const N: usize> {
            head: choice_ty!($padded, CachePadded<AtomicUsize>, AtomicUsize),
            tail: choice_ty!($padded, CachePadded<AtomicUsize>, AtomicUsize),
            $(
                // TODO: measure whether CachePadded helps
                $skip: choice_ty!($padded, CachePadded<AtomicUsize>, AtomicUsize),
            )?
            flags: AtomicU8,
            /// The static array holding slots.
            ///
            /// This must be in an `UnsafeCell` because both producer and consumer
            /// have a (non-mutable) reference to the ring buffer and they use
            /// *interior mutability* to modify it.
            slots: UnsafeCell<[MaybeUninit<T>; N]>,
        }

        // SAFETY: If T can be moved between threads, RingBuffer can as well.
        unsafe impl<T: Send, const N: usize> Send for RingBuffer<T, N> {}

        impl<T, const N: usize> RingBuffer<T, N> {
            // Private helper function.
            const fn construct() -> Self {
                Self {
                    head: choice!($padded,
                        CachePadded::new(AtomicUsize::new(0)),
                        AtomicUsize::new(0)),
                    tail: choice!($padded,
                        CachePadded::new(AtomicUsize::new(0)),
                        AtomicUsize::new(0)),
                    $(
                        $skip: choice!($padded,
                            CachePadded::new(AtomicUsize::new(N)),
                            AtomicUsize::new(N)),
                    )?
                    flags: AtomicU8::new(0),
                    slots: UnsafeCell::new([const { MaybeUninit::uninit() }; N]),
                }
            }

            fn data_ptr(&self) -> *mut T {
                // TODO: what happens if N == 0?
                self.slots.get().cast()
            }

            fn capacity(&self) -> usize {
                N
            }
        }

        impl<T, const N: usize> Drop for RingBuffer<T, N> {
            fn drop(&mut self) {
                // SAFETY: this is called exactly once, no references to any elements exist anymore.
                unsafe { self.drop_all_elements() };
            }
        }

        storage_array_impl_default_for_ring_buffer!(arc = $arc);
    };
}

macro_rules! storage_array_impl_default_for_ring_buffer {
    (arc = yes) => {};
    (arc = no) => {
        impl<T, const N: usize> Default for RingBuffer<T, N> {
            fn default() -> Self {
                Self::new()
            }
        }
    };
}

// TODO: check that `bip = yes` is not allowed?
// TODO: check that `contiguous = yes` is required?
// TODO: only allow `pow2 = yes`?
#[cfg(feature = "vrb")]
macro_rules! storage_vrb {
    (padded = $padded:ident, rb_doc = $rb_doc:expr) => {
        #[doc = $rb_doc]
        // TODO: reuse from storage_vec, disabling "skip"?
        pub struct RingBuffer<T> {
            head: choice_ty!($padded, CachePadded<AtomicUsize>, AtomicUsize),
            tail: choice_ty!($padded, CachePadded<AtomicUsize>, AtomicUsize),
            flags: AtomicU8,
            /// Pointer to the first mapped region
            data_ptr: *mut T,
            capacity: usize,
        }

        impl<T> RingBuffer<T> {
            // Private helper function.
            fn construct(capacity: usize) -> Self {
                use core::mem;
                const {
                    // NB: This also disallows zero-sized types,
                    //     and therefore avoids division by zero further below:
                    assert!(
                        mem::size_of::<T>().is_power_of_two(),
                        "size of T must be a power of 2"
                    );
                }
                // TODO: what if capacity is 0?
                // SAFETY: If `libc` is not buggy, this should be safe.
                let pagesize = unsafe { libc::sysconf(libc::_SC_PAGESIZE) };
                assert_ne!(pagesize, -1);
                let pagesize = usize::try_from(pagesize).unwrap();
                assert!(pagesize.is_power_of_two());
                assert!(pagesize >= mem::size_of::<T>());
                assert_eq!(pagesize % mem::size_of::<T>(), 0);
                let elements_per_page = pagesize / mem::size_of::<T>();
                let pages = capacity.div_ceil(elements_per_page);
                let capacity = pages * elements_per_page;
                assert_eq!(capacity, Self::update_capacity(capacity));
                let len = capacity * mem::size_of::<T>();

                // SAFETY:
                // - string is null-terminated
                // - pointers, lengths and other arguments are valid
                let data_ptr: *mut T = unsafe {
                    use libc::*;
                    let mut filename = *b"/tmp/rtrb-buffer-XXXXXX\0";
                    let filename = filename.as_mut_ptr().cast();
                    let fd = mkstemp(filename);
                    assert!(fd >= 0);
                    let r = unlink(filename);
                    assert_eq!(r, 0);
                    let r = ftruncate(fd, off_t::try_from(len).unwrap());
                    assert_eq!(r, 0);
                    // Get an address with twice the capacity available
                    let ptr_one = mmap(
                        core::ptr::null_mut(),
                        2 * len,
                        PROT_NONE,
                        MAP_PRIVATE | MAP_ANONYMOUS,
                        -1,
                        0,
                    );
                    assert_ne!(ptr_one, MAP_FAILED); // TODO: check for errno?
                    let r = mmap(
                        ptr_one,
                        len,
                        PROT_READ | PROT_WRITE,
                        MAP_SHARED | MAP_FIXED,
                        fd,
                        0,
                    );
                    assert_eq!(r, ptr_one); // TODO: check for errno?
                    let ptr_two = ptr_one.add(len);
                    let r = mmap(
                        ptr_two,
                        len,
                        PROT_READ | PROT_WRITE,
                        MAP_SHARED | MAP_FIXED,
                        fd,
                        0,
                    );
                    assert_eq!(r, ptr_two); // TODO: check for errno?
                    let r = close(fd);
                    assert_eq!(r, 0); // TODO: check for errno?
                    ptr_one.cast()
                };
                // Alignments larger than the page size are not supported.
                assert!(data_ptr.is_aligned());
                // TODO: reuse from storage_vec, disabling "skip"?
                Self {
                    head: choice!(
                        $padded,
                        CachePadded::new(AtomicUsize::new(0)),
                        AtomicUsize::new(0)
                    ),
                    tail: choice!(
                        $padded,
                        CachePadded::new(AtomicUsize::new(0)),
                        AtomicUsize::new(0)
                    ),
                    flags: AtomicU8::new(0),
                    data_ptr,
                    capacity,
                }
            }

            // TODO: reuse capacity() and data_ptr() from storage_vec?

            fn capacity(&self) -> usize {
                self.capacity
            }

            fn data_ptr(&self) -> *mut T {
                self.data_ptr
            }
        }

        impl<T> Drop for RingBuffer<T> {
            /// Drops all non-empty slots and deallocates the storage.
            fn drop(&mut self) {
                // SAFETY: this is called exactly once, no references to any elements exist anymore.
                unsafe { self.drop_all_elements() };
                // SAFETY: The memory is not used anymore.
                unsafe {
                    let len = self.capacity() * core::mem::size_of::<T>();
                    let ptr_one: *mut libc::c_void = self.data_ptr.cast();
                    let r = libc::munmap(ptr_one, len);
                    assert_eq!(r, 0); // TODO: check for errno?
                    let ptr_two = ptr_one.add(len);
                    let r = libc::munmap(ptr_two, len);
                    assert_eq!(r, 0); // TODO: check for errno?
                }
            }
        }
    };
}

macro_rules! choice {
    (yes, $(#[doc = $yes:expr])* :: $(#[doc = $no:expr])*) => {
        docstring!($(#[doc = $yes])*)
    };
    (no, $(#[doc = $yes:expr])* :: $(#[doc = $no:expr])*) => {
        docstring!($(#[doc = $no])*)
    };
    (yes, $yes:expr, $no:expr) => {
        $yes
    };
    (no, $yes:expr, $no:expr) => {
        $no
    };
}

macro_rules! choice_ty {
    (yes, $yes:ty, $no:ty) => {
        $yes
    };
    (no, $yes:ty, $no:ty) => {
        $no
    };
}

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
        check_bip_contiguous!($bip, $contiguous);

        use core::{cell::Cell, fmt};

        use $crate::atomic::*;

        // TODO: import only if `padded = yes`?
        // Padded indices to avoid false sharing.
        #[allow(unused_imports)]
        use $crate::CachePadded;

        // TODO: import only when appropriate?
        #[allow(unused_imports)]
        use $crate::{HAS_CONSUMER, HAS_PRODUCER};
        #[allow(unused_imports)]
        #[cfg(feature = "alloc")]
        use $crate::IS_ABANDONED;

        /// Error type for [`Consumer::peek()`].
        #[doc(inline)]
        pub use $crate::PeekError;
        /// Error type for [`Consumer::pop()`].
        #[doc(inline)]
        pub use $crate::PopError;
        /// Error type for [`Producer::push()`].
        #[doc(inline)]
        pub use $crate::PushError;
        /// Error type for [`Producer::write_chunk()`], [`Producer::write_chunk_uninit()`]
        /// and [`Consumer::read_chunk()`].
        ///
        /// To get the maximum number of available slots beforehand
        /// (and therefore avoid this error), use
        #[doc = choice!($bip,
            /// [`Producer::slots_contiguous_max()`] and [`Consumer::slots_contiguous_first()`],
            ::
            /// [`Producer::slots()`] and [`Consumer::slots()`],
        )]
        /// respectively.
        #[doc(inline)]
        pub use $crate::ChunkError;

        /// Extension trait providing a [`copy_to_uninit()`](CopyToUninit::copy_to_uninit)
        /// method on built-in slices.
        ///
        /// This can be used to safely copy data to the
        #[doc = choice!($contiguous,
            /// slice returned from [`WriteChunkUninit::as_mut_slice()`].
            ::
            /// slices returned from [`WriteChunkUninit::as_mut_slices()`].
        )]
        ///
        /// To use this, the trait has to be brought into scope, e.g. with:
        ///
        /// ```
        #[doc = concat!("use ", $module, "::CopyToUninit as _;")]
        /// ```
        ///
        /// TODO: update link:
        ///
        /// For a usage example, see [`crate::chunks`](crate::chunks#common-access-patterns).
        #[doc(inline)]
        pub use $crate::CopyToUninit;

        storage!(storage = $storage, arc = $arc, bip = $bip, padded = $padded, rb_doc = $rb_doc);

        // SAFETY: RingBuffer is only mutated (using *interior mutablility*)
        // via Producer/Consumer (which are !Sync), all other access can be shared.
        impl_!(
            /// A `RingBuffer` can be shared between threads.
            ///
            /// `T` does not need to be `Sync`, because we never share it across threads.
            RingBuffer, trait unsafe = Sync, N = $N, where T: Send {});

        // NB: `Send` might be implemented by different storage backends,
        // but it is not necessary for correct behavior of the RingBuffer.

        impl_!(RingBuffer, N = $N, {
            fn_ring_buffer_new!(N = $N, arc = $arc, pow2 = $pow2, module = $module);
            fn_ring_buffer_producer!(N = $N, arc = $arc, module = $module);
            fn_ring_buffer_consumer!(N = $N, arc = $arc, module = $module);
            fn_ring_buffer_has_producer!(N = $N, arc = $arc);
            fn_ring_buffer_has_consumer!(N = $N, arc = $arc);

            fn_ring_buffer_drop_all_elements!(bip = $bip);
            fn_ring_buffer_update_capacity!(pow2 = $pow2);
            fn_ring_buffer_collapse_position!(pow2 = $pow2);
            fn_ring_buffer_slot_ptr!();
            fn_ring_buffer_increment!(pow2 = $pow2);
            fn_ring_buffer_increment1!(pow2 = $pow2);
            fn_ring_buffer_distance!(pow2 = $pow2);
        });

        impl_!(RingBuffer, trait = PartialEq, N = $N, {
            fn eq(&self, other: &Self) -> bool {
                core::ptr::eq(self, other)
            }
        });
        impl_!(RingBuffer, trait = Eq, N = $N, {});

        struct_arc_ring_buffer!(N = $N, arc = $arc);

        struct_producer!(N = $N, arc = $arc);
        impl_send_for_producer!(N = $N, arc = $arc, module = $module);

        impl_!(Producer, trait = PartialEq, N = $N, arc = $arc, {
            fn eq(&self, other: &Self) -> bool {
                self.buffer == other.buffer
            }
        });

        impl_!(Producer, N = $N, arc = $arc, {
            fn_producer_push!(storage = $storage, N = $N, arc = $arc, module = $module);
            fn_producer_write_chunk!(N = $N, bip = $bip, contiguous = $contiguous);
            fn_producer_write_chunk_uninit!(N = $N, bip = $bip, contiguous = $contiguous);
            fn_producer_slots!(N = $N, arc = $arc, bip = $bip, module = $module);
            fn_producer_slots_contiguousX!(bip = $bip);
            fn_producer_is_full!(storage = $storage, N = $N, arc = $arc, bip = $bip, module = $module);
            fn_pc_capacity!(N = $N, arc = $arc, bip = $bip, module = $module);
            fn_producer_is_abandoned!(N = $N, arc = $arc, module = $module);
            fn_producer_has_consumer!(N = $N, arc = $arc, module = $module);

            fn_producer_next_tail!();
        });

        struct_consumer!(N = $N, arc = $arc);
        impl_send_for_consumer!(N = $N, arc = $arc, module = $module);

        // TODO: code reuse with Producer
        impl_!(Consumer, trait = PartialEq, N = $N, arc = $arc, {
            fn eq(&self, other: &Self) -> bool {
                self.buffer == other.buffer
            }
        });

        impl_!(Consumer, N = $N, arc = $arc, {
            fn_consumer_pop!(N = $N, arc = $arc, module = $module);
            fn_consumer_peek!(N = $N, arc = $arc, module = $module);
            fn_consumer_read_chunk!(N = $N, bip = $bip, contiguous = $contiguous);
            fn_consumer_slots!(N = $N, arc = $arc, bip = $bip, module = $module);
            fn_consumer_slots_contiguousX!(bip = $bip);
            fn_consumer_is_empty!(N = $N, arc = $arc, module = $module);
            fn_pc_capacity!(N = $N, arc = $arc, bip = $bip, module = $module);
            fn_consumer_is_abandoned!(N = $N, arc = $arc, module = $module);
            fn_consumer_has_producer!(N = $N, arc = $arc, module = $module);

            fn_consumer_next_head!(bip = $bip);
        });

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

        #[doc = mod_chunks_docstring!(storage = $storage, N = $N, arc = $arc, contiguous = $contiguous, module = $module)]
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

                fn_read_chunk_uninit_commit_unchecked!(contiguous = $contiguous);
            });

            impl_debug!(N = $N,
                WriteChunkUninit<'_>, WriteChunk<'_>, ReadChunk<'_>, ReadChunkIntoIter<'_>);

            impl_into_iterator_for_read_chunk!(N = $N, contiguous = $contiguous);
        }

        use chunks::{ReadChunk, WriteChunk, WriteChunkUninit};
    };
}

macro_rules! check_bip_contiguous {
    (yes, no) => {
        compile_error!("`bip = yes` requires `contiguous = yes`");
    };
    (no, yes) => {
        // This is used for "vrb".
    };
    (yes, yes) => {};
    (no, no) => {};
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

macro_rules! doctest_create_ring_buffer {
    (N = yes, arc = yes, capacity = $capacity:literal$(, $prefix:literal)?) => {
        concat!(
            $($prefix, )?
            "let (mut p, mut c) = RingBuffer::<_, ",
            $capacity,
            ">::new();"
        )
    };
    (N = no, arc = yes, capacity = $capacity:literal$(, $prefix:literal)?) => {
        concat!($($prefix, )?"let (mut p, mut c) = RingBuffer::new(", $capacity, ");")
    };
    (N = yes, arc = no, capacity = $capacity:literal$(, $prefix:literal)?) => {
        concat!(
            $($prefix, )?
            "let rb = RingBuffer::<_, ",
            $capacity,
            ">::new();\n",
            $($prefix, )?
            "let mut p = rb.producer().unwrap();\n",
            $($prefix, )?
            "let mut c = rb.consumer().unwrap();",
        )
    };
    (N = no, arc = no, capacity = $capacity:literal$(, $prefix:literal)?) => {
        concat!(
            $($prefix, )?
            "let rb = RingBuffer::new(",
            $capacity,
            ");\n",
            $($prefix, )?
            "let mut p = rb.producer().unwrap();\n",
            $($prefix, )?
            "let mut c = rb.consumer().unwrap();",
        )
    };
}

macro_rules! doctest_ty {
    ($name:ident, $ty:ty, $N:literal, N = yes) => {
        concat!(stringify!($name), "<", stringify!($ty), ", ", $N, ">")
    };
    ($name:ident, $ty:ty, $N:literal, N = no) => {
        concat!(stringify!($name), "<", stringify!($ty), ">")
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

#[rustfmt::skip] // https://github.com/rust-lang/rustfmt/issues/5974
macro_rules! fn_ring_buffer_new {
    (N = yes, arc = yes, pow2 = $pow2:ident, module = $module:literal) => {
        /// Creates a ring buffer with a capacity of `N` and returns [`Producer`] and [`Consumer`].
        ///
        #[doc = choice!($pow2, "`N` must be a power of two.", "")]
        ///
        /// # Examples
        ///
        /// ```
        #[doc = doctest_import!($module, "RingBuffer")]
        ///
        /// let (p, c) = RingBuffer::<f32, 128>::new();
        /// ```
        ///
        /// Specifying an explicit type
        /// is is only necessary if it cannot be deduced by the compiler.
        ///
        /// ```
        #[doc = doctest_import!($module, "RingBuffer")]
        ///
        /// let (mut p, c) = RingBuffer::<_, 128>::new();
        /// assert_eq!(p.push(0.0f32), Ok(()));
        /// ```
        #[allow(clippy::new_ret_no_self)]
        pub fn new() -> (Producer<T, N>, Consumer<T, N>) {
            const {
                assert!(Self::update_capacity(N) == N, "`N` must be a power of two");
            }
            ArcRingBuffer::new(Self::construct())
        }
    };
    (N = yes, arc = no, pow2 = $pow2:ident, module = $module:literal) => {
        /// Creates a ring buffer with a capacity of `N`.
        ///
        #[doc = choice!($pow2, "`N` must be a power of two.", "")]
        ///
        /// A (single) [`Producer`] for writing into the ring buffer can be created with
        /// [`producer()`](RingBuffer::producer).
        /// A (single) [`Consumer`] for reading from the ring buffer can be created with
        /// [`consumer()`](RingBuffer::consumer).
        ///
        /// # Examples
        ///
        /// ```
        #[doc = doctest_import!($module, "RingBuffer")]
        ///
        /// let rb = RingBuffer::<f32, 128>::new();
        /// ```
        ///
        /// Specifying an explicit type
        /// is is only necessary if it cannot be deduced by the compiler.
        ///
        /// ```
        #[doc = doctest_import!($module, "RingBuffer")]
        ///
        /// let rb = RingBuffer::<_, 128>::new();
        /// let mut p = rb.producer().unwrap();
        /// assert_eq!(p.push(0.0f32), Ok(()));
        /// ```
        pub const fn new() -> Self {
            const {
                assert!(Self::update_capacity(N) == N, "`N` must be a power of two");
            }
            Self::construct()
        }
    };
    (N = no, arc = yes, pow2 = $pow2:ident, module = $module:literal) => {
        /// Creates a ring buffer
        #[doc = choice!($pow2, "with at least", "with")]
        /// the given `capacity` and returns [`Producer`] and [`Consumer`].
        ///
        #[doc = choice!($pow2,
            /// If the `capacity` isn't already a power of two, it is rounded up to the next one.
            ::
        )]
        ///
        /// # Examples
        ///
        /// ```
        #[doc = doctest_import!($module, "RingBuffer")]
        ///
        /// let (p, c) = RingBuffer::<f32>::new(100);
        /// ```
        ///
        /// Specifying an explicit type with the [turbofish](https://turbo.fish/)
        /// is is only necessary if it cannot be deduced by the compiler.
        ///
        /// ```
        #[doc = doctest_import!($module, "RingBuffer")]
        ///
        /// let (mut p, c) = RingBuffer::new(100);
        /// assert_eq!(p.push(0.0f32), Ok(()));
        /// ```
        #[allow(clippy::new_ret_no_self)]
        pub fn new(capacity: usize) -> (Producer<T>, Consumer<T>) {
            let capacity = Self::update_capacity(capacity);
            ArcRingBuffer::new(Self::construct(capacity))
        }
    };
    (N = no, arc = no, pow2 = $pow2:ident, module = $module:literal) => {
        /// Creates a ring buffer
        #[doc = choice!($pow2, "with at least", "with")]
        /// the given `capacity`.
        ///
        #[doc = choice!($pow2,
            /// If the `capacity` isn't already a power of two, it is rounded up to the next one.
            ::
        )]
        ///
        /// A (single) [`Producer`] for writing into the ring buffer can be created with
        /// [`producer()`](RingBuffer::producer).
        /// A (single) [`Consumer`] for reading from the ring buffer can be created with
        /// [`consumer()`](RingBuffer::consumer).
        ///
        /// # Examples
        ///
        /// ```
        #[doc = doctest_import!($module, "RingBuffer")]
        ///
        /// let rb = RingBuffer::<f32>::new(100);
        /// ```
        ///
        /// Specifying an explicit type with the [turbofish](https://turbo.fish/)
        /// is is only necessary if it cannot be deduced by the compiler.
        ///
        /// ```
        #[doc = doctest_import!($module, "RingBuffer")]
        ///
        /// let rb = RingBuffer::new(100);
        /// let mut p = rb.producer().unwrap();
        /// assert_eq!(p.push(0.0f32), Ok(()));
        /// ```
        pub fn new(capacity: usize) -> Self {
            let capacity = Self::update_capacity(capacity);
            Self::construct(capacity)
        }
    };
}

macro_rules! fn_ring_buffer_drop_all_elements {
    (bip = yes) => {
        fn_ring_buffer_drop_all_elements_helper!(skip);
    };
    (bip = no) => {
        fn_ring_buffer_drop_all_elements_helper!();
    };
}
macro_rules! fn_ring_buffer_drop_all_elements_helper {
    ($($skip:ident)?) => {
        /// Drop all elements that are still in the buffer.
        ///
        /// After this, head and tail indices are invalid.
        ///
        /// # Safety
        ///
        /// This can only be called in the `Drop` implementation of the ring buffer.
        ///
        /// The threads must have been synchronized before via `self.flags`.
        #[inline(never)]
        unsafe fn drop_all_elements(&mut self) {
            // These atomic variables are *not* used for synchronizing the threads
            // before destruction.  Relaxed ordering is sufficient here.
            let mut head = self.head.load(Ordering::Relaxed);
            let tail = self.tail.load(Ordering::Relaxed);
            $(
                let $skip = self.skip.load(Ordering::Relaxed);
            )?
            // Loop over all slots that hold a value and drop them.
            while head != tail {
                $(
                    if self.collapse_position(head) == $skip {
                        head = self.increment(head, self.capacity() - $skip);
                    }
                )?
                // SAFETY: All slots between head and tail have been initialized.
                unsafe { self.slot_ptr(head).drop_in_place() };
                head = self.increment1(head);
            }
        }
    };
}

macro_rules! fn_ring_buffer_update_capacity {
    (pow2 = yes) => {
        const fn update_capacity(capacity: usize) -> usize {
            capacity.next_power_of_two()
        }
    };
    (pow2 = no) => {
        const fn update_capacity(capacity: usize) -> usize {
            capacity
        }
    };
}

/// Makes sure the position is in the range `0 .. capacity`.
macro_rules! fn_ring_buffer_collapse_position {
    (pow2 = yes) => {
        // Wraps from any number to the range `0 .. capacity`.
        fn collapse_position(&self, pos: usize) -> usize {
            // TODO: is capacity 0 supported?
            pos & (self.capacity() - 1)
        }
    };
    (pow2 = no) => {
        // Wraps from the range `0 .. 2 * capacity` to `0 .. capacity`.
        fn collapse_position(&self, pos: usize) -> usize {
            debug_assert!(pos == 0 || pos < 2 * self.capacity());
            if pos < self.capacity() {
                pos
            } else {
                pos - self.capacity()
            }
        }
    };
}

macro_rules! fn_ring_buffer_slot_ptr {
    () => {
        /// Returns a pointer to the (possibly uninitialized) slot at position `pos`.
        ///
        /// # Safety
        ///
        /// `pos` must be valid.
        ///
        /// If `pos == 0 && capacity == 0`, the returned pointer must not be dereferenced!
        unsafe fn slot_ptr(&self, pos: usize) -> *mut T {
            // SAFETY: See docstring.
            unsafe { self.data_ptr().add(self.collapse_position(pos)) }
        }
    };
}

/// Increments a position by going `n` slots forward.
macro_rules! fn_ring_buffer_increment {
    (pow2 = yes) => {
        fn increment(&self, pos: usize, n: usize) -> usize {
            pos.wrapping_add(n)
        }
    };
    (pow2 = no) => {
        fn increment(&self, pos: usize, n: usize) -> usize {
            debug_assert!(pos == 0 || pos < 2 * self.capacity());
            debug_assert!(n <= self.capacity());
            let threshold = 2 * self.capacity() - n;
            if pos < threshold {
                pos + n
            } else {
                pos - threshold
            }
        }
    };
}

/// Increments a position by going one slot forward.
///
/// This might be more efficient than self.increment(..., 1).
macro_rules! fn_ring_buffer_increment1 {
    (pow2 = yes) => {
        fn increment1(&self, pos: usize) -> usize {
            pos.wrapping_add(1)
        }
    };
    (pow2 = no) => {
        /// Increments a position by going one slot forward.
        ///
        /// This might be more efficient than self.increment(..., 1).
        fn increment1(&self, pos: usize) -> usize {
            debug_assert_ne!(self.capacity(), 0);
            debug_assert!(pos < 2 * self.capacity());
            if pos < 2 * self.capacity() - 1 {
                pos + 1
            } else {
                0
            }
        }
    };
}

/// Returns the distance between two positions.
macro_rules! fn_ring_buffer_distance {
    (pow2 = yes) => {
        fn distance(&self, a: usize, b: usize) -> usize {
            b.wrapping_sub(a)
        }
    };
    (pow2 = no) => {
        fn distance(&self, a: usize, b: usize) -> usize {
            debug_assert!(a == 0 || a < 2 * self.capacity());
            debug_assert!(b == 0 || b < 2 * self.capacity());
            if a <= b {
                b - a
            } else {
                2 * self.capacity() - a + b
            }
        }
    };
}

macro_rules! struct_arc_ring_buffer {
    (N = $N:ident, arc = yes) => {
        use alloc::boxed::Box;
        use core::ptr::NonNull;

        // Non-public helper type.
        struct_!(ArcRingBuffer, N = $N, {
            ptr: NonNull<generic!(RingBuffer, N = $N)>,
        });

        // SAFETY: If RingBuffer is Send, ArcRingBuffer is as well.
        impl_!(
            ArcRingBuffer,
            trait unsafe = Send,
            N = $N,
            where generic!(RingBuffer, N = $N): Send {});

        impl_!(ArcRingBuffer, N = $N, {
            // NB: this takes ownership of the RingBuffer, making sure that only one
            //     Producer and Consumer are ever created.
            #[allow(clippy::new_ret_no_self)]
            fn new(
                rb: generic!(RingBuffer, N = $N)
            ) -> (generic!(Producer, N = $N), generic!(Consumer, N = $N)) {
                debug_assert_eq!(rb.flags.load(Ordering::Relaxed) & IS_ABANDONED, 0);
                let head = rb.head.load(Ordering::Relaxed);
                let tail = rb.tail.load(Ordering::Relaxed);
                let ptr = Box::leak(Box::new(rb));
                // SAFETY: Pointer from Box is always non-null.
                let ptr = unsafe { NonNull::new_unchecked(ptr) };
                let p = Producer {
                    buffer: Self { ptr },
                    cached_head: Cell::new(head),
                    cached_tail: Cell::new(tail),
                };
                let c = Consumer {
                    buffer: Self { ptr },
                    cached_head: Cell::new(head),
                    cached_tail: Cell::new(tail),
                };
                (p, c)
            }
        });

        impl_!(ArcRingBuffer, trait = Drop, N = $N, {
            fn drop(&mut self) {
                // SAFETY: must point to initialized Storage.
                let flags: &AtomicU8 = unsafe { &self.ptr.as_ref().flags };
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
                    // SAFETY: RingBuffer has been allocated with `Box::new()`.
                    unsafe {
                        drop_slow(self.ptr);
                    }
                }
            }
        });

        fn_arc_ring_buffer_drop_slow!(N = $N);

        impl_!(ArcRingBuffer, trait = core::ops::Deref, N = $N, {
            type Target = generic!(RingBuffer, N = $N);

            fn deref(&self) -> &Self::Target {
                // SAFETY: There are never any mutable references.
                unsafe { self.ptr.as_ref() }
            }
        });

        impl_!(ArcRingBuffer, trait = PartialEq, N = $N, {
            fn eq(&self, other: &Self) -> bool {
                self.ptr == other.ptr
            }
        });
    };
    (N = $N:ident, arc = no) => {};
}

#[cfg(feature = "alloc")]
macro_rules! fn_arc_ring_buffer_drop_slow_helper {
    (params = ($($params:tt)*), args = ($($args:tt)*)) => {
        /// Non-inlined part of `Ref::drop()`.
        #[inline(never)]
        unsafe fn drop_slow<$($params)*>(ptr: NonNull<RingBuffer<$($args)*>>) {
            // SAFETY: This is allowed because the storage has been allocated with `Box::new()`.
            unsafe {
                // Turn the pointer back into a `Box` and immediately drop it,
                // which deallocates the memory allocated in `Ref::new()`.
                drop(Box::from_raw(ptr.as_ptr()));
            }
        }
    };
}

#[cfg(feature = "alloc")]
macro_rules! fn_arc_ring_buffer_drop_slow {
    (N = yes) => {
        fn_arc_ring_buffer_drop_slow_helper!(params = (T, const N: usize), args = (T, N));
    };
    (N = no) => {
        fn_arc_ring_buffer_drop_slow_helper!(params = (T), args = (T));
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

macro_rules! fn_ring_buffer_producer {
    (N = $N:ident, arc = yes, module = $module:literal) => {};
    (N = $N:ident, arc = no, module = $module:literal) => {
        /// Creates a [`Producer`] (if it doesn't exist yet) for writing into the `RingBuffer`.
        ///
        /// # Examples
        ///
        /// Only one producer and one consumer can exist at once,
        /// but once a producer has been dropped, a new one can be created:
        /// ```
        #[doc = doctest_import!($module, "RingBuffer")]
        ///
        #[doc = doctest_create_ring_buffer!(N = $N, arc = no, capacity = 64)]
        /// assert!(rb.producer().is_none());
        /// assert_eq!(p.push(10), Ok(()));
        /// drop(p);
        /// assert!(!c.has_producer());
        /// assert!(!rb.has_producer());
        /// let mut p = rb.producer().unwrap();
        /// assert!(c.has_producer());
        /// assert!(rb.has_producer());
        /// assert_eq!(p.push(20), Ok(()));
        /// assert_eq!(c.pop(), Ok(10));
        /// assert_eq!(c.pop(), Ok(20));
        /// ```
        pub fn producer(&self) -> Option<generic!(Producer<'_>, N = $N)> {
            let old_flags = self.flags.fetch_or(HAS_PRODUCER, Ordering::SeqCst);
            if old_flags & HAS_PRODUCER == 0 {
                let head = self.head.load(Ordering::Relaxed);
                let tail = self.tail.load(Ordering::Relaxed);
                Some(Producer {
                    buffer: self,
                    cached_head: Cell::new(head),
                    cached_tail: Cell::new(tail),
                })
            } else {
                None
            }
        }
    };
}

macro_rules! fn_ring_buffer_consumer {
    (N = $N:ident, arc = yes, module = $module:literal) => {};
    (N = $N:ident, arc = no, module = $module:literal) => {
        /// Creates a [`Consumer`] (if it doesn't exist yet) for reading from the `RingBuffer`.
        ///
        /// # Examples
        ///
        /// Only one producer and one consumer can exist at once,
        /// but once a consumer has been dropped, a new one can be created:
        /// ```
        #[doc = doctest_import!($module, "RingBuffer")]
        ///
        #[doc = doctest_create_ring_buffer!(N = $N, arc = no, capacity = 64)]
        /// assert!(rb.consumer().is_none());
        /// assert_eq!(p.push(10), Ok(()));
        /// assert_eq!(p.push(20), Ok(()));
        /// assert_eq!(c.pop(), Ok(10));
        /// drop(c);
        /// assert!(!p.has_consumer());
        /// assert!(!rb.has_consumer());
        /// let mut c = rb.consumer().unwrap();
        /// assert!(p.has_consumer());
        /// assert!(rb.has_consumer());
        /// assert_eq!(c.pop(), Ok(20));
        /// ```
        pub fn consumer(&self) -> Option<generic!(Consumer<'_>, N = $N)> {
            let old_flags = self.flags.fetch_or(HAS_CONSUMER, Ordering::SeqCst);
            if old_flags & HAS_CONSUMER == 0 {
                let head = self.head.load(Ordering::Relaxed);
                let tail = self.tail.load(Ordering::Relaxed);
                Some(Consumer {
                    buffer: self,
                    cached_head: Cell::new(head),
                    cached_tail: Cell::new(tail),
                })
            } else {
                None
            }
        }
    };
}

macro_rules! fn_ring_buffer_has_producer {
    (N = $N:ident, arc = yes) => {};
    (N = $N:ident, arc = no) => {
        /// Returns `true` if a [`Producer`] exists for this `RingBuffer`.
        ///
        /// If not, it can be created with [`producer()`](RingBuffer::producer).
        ///
        /// See also [`Consumer::has_producer()`].
        pub fn has_producer(&self) -> bool {
            self.flags.load(Ordering::SeqCst) & HAS_PRODUCER != 0
        }
    };
}

macro_rules! fn_ring_buffer_has_consumer {
    (N = $N:ident, arc = yes) => {};
    (N = $N:ident, arc = no) => {
        /// Returns `true` if a [`Consumer`] exists for this `RingBuffer`.
        ///
        /// If not, it can be created with [`consumer()`](RingBuffer::consumer).
        ///
        /// See also [`Producer::has_consumer()`].
        pub fn has_consumer(&self) -> bool {
            self.flags.load(Ordering::SeqCst) & HAS_CONSUMER != 0
        }
    };
}

macro_rules! struct_producer_docstring {
    () => { docstring!(
        /// The producer side of a [`RingBuffer`].
        ///
        /// A `Producer` can be moved between threads,
        /// but references from different threads are not allowed
        /// (i.e. it is [`Send`] but not [`Sync`]).
        ///
        /// Individual elements can be moved into the ring buffer with [`push()`](Producer::push),
        /// multiple elements at once can be written with [`write_chunk()`](Producer::write_chunk)
        /// and [`write_chunk_uninit()`](Producer::write_chunk_uninit).
        ///
        /// The number of free slots currently available for writing can be obtained with
        /// [`slots()`](Producer::slots).
    )};
}

macro_rules! struct_producer {
    (N = $N:ident, arc = yes) => {
        struct_!(
            #[doc = struct_producer_docstring!()]
            ///
            /// A `Producer` can only be created with [`RingBuffer::new()`]
            /// (together with its counterpart, the [`Consumer`]).
            ///
            /// When the `Producer` is dropped, [`Consumer::is_abandoned()`] will return `true`.
            /// This can be used as a crude way to communicate to the receiving thread
            /// that no more data will be produced.
            /// When the `Producer` is dropped after the [`Consumer`] has already been dropped,
            /// all items remaining in the ring buffer will be dropped and the allocated memory
            /// will be deallocated.
            pub Producer, N = $N, {
                buffer: generic!(ArcRingBuffer, N = $N),
                /// A copy of `buffer.head` for quick access.
                ///
                /// This value can be stale and sometimes needs to be resynchronized
                /// with `buffer.head`.
                cached_head: Cell<usize>,
                /// A copy of `buffer.tail` for quick access.
                ///
                /// This value is always in sync with `buffer.tail`.
                cached_tail: Cell<usize>,
            }
        );
    };
    (N = $N:ident, arc = no) => {
        // TODO: manual impls:
        //#[derive(Debug, PartialEq, Eq)]
        struct_!(
            #[doc = struct_producer_docstring!()]
            ///
            /// A `Producer` can only be created with [`RingBuffer::producer()`].
            pub Producer<'a>, N = $N, {
                buffer: &'a generic!(RingBuffer, N = $N),
                cached_head: Cell<usize>,
                cached_tail: Cell<usize>,
            }
        );

        impl_!(Producer<'_>, trait = Drop, N = $N, {
            fn drop(&mut self) {
                let _ = self.buffer.flags.fetch_and(!HAS_PRODUCER, Ordering::SeqCst);
            }
        });
    };
}

macro_rules! struct_consumer_docstring {
    () => { docstring!(
        /// The consumer side of a [`RingBuffer`].
        ///
        /// A `Consumer` can be moved between threads,
        /// but references from different threads are not allowed
        /// (i.e. it is [`Send`] but not [`Sync`]).
        ///
        /// Individual elements can be moved out of the ring buffer with [`pop()`](Consumer::pop),
        /// multiple elements at once can be read with [`read_chunk()`](Consumer::read_chunk).
        ///
        /// The number of slots currently available for reading can be obtained with
        /// [`slots()`](Consumer::slots).
    )};
}

macro_rules! struct_consumer {
    (N = $N:ident, arc = yes) => {
        // TODO: manual impls:
        //#[derive(Debug, PartialEq, Eq)]
        struct_!(
            #[doc = struct_consumer_docstring!()]
            ///
            /// A `Consumer` can only be created with [`RingBuffer::new()`]
            /// (together with its counterpart, the [`Producer`]).
            ///
            /// When the `Consumer` is dropped, [`Producer::is_abandoned()`] will return `true`.
            /// This can be used as a crude way to communicate to the sending thread
            /// that no more data will be consumed.
            /// When the `Consumer` is dropped after the [`Producer`] has already been dropped,
            /// all items remaining in the ring buffer will be dropped and the allocated memory
            /// will be deallocated.
            pub Consumer, N = $N, {
                buffer: generic!(ArcRingBuffer, N = $N),
                /// A copy of `buffer.head` for quick access.
                ///
                /// This value is always in sync with `buffer.head`.
                cached_head: Cell<usize>,
                /// A copy of `buffer.tail` for quick access.
                ///
                /// This value can be stale and sometimes needs to be resynchronized
                /// with `buffer.tail`.
                cached_tail: Cell<usize>,
            }
        );
    };
    (N = $N:ident, arc = no) => {
        // TODO: manual impls:
        //#[derive(Debug, PartialEq, Eq)]
        struct_!(
            #[doc = struct_consumer_docstring!()]
            ///
            /// A `Consumer` can only be created with [`RingBuffer::consumer()`].
            pub Consumer<'a>, N = $N, {
                buffer: &'a generic!(RingBuffer, N = $N),
                cached_head: Cell<usize>,
                cached_tail: Cell<usize>,
            }
        );

        impl_!(Consumer<'_>, trait = Drop, N = $N, {
            fn drop(&mut self) {
                let _ = self.buffer.flags.fetch_and(!HAS_CONSUMER, Ordering::SeqCst);
            }
        });
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

#[rustfmt::skip] // https://github.com/rust-lang/rustfmt/issues/5974
macro_rules! fn_producer_push {
    (storage = $storage:ident, N = $N:ident, arc = $arc:ident, module = $module:literal) => {
        /// Attempts to push an element into the queue.
        ///
        /// The element is *moved* into the ring buffer and its slot
        /// is made available to be read by the [`Consumer`].
        ///
        /// # Errors
        ///
        /// If the queue is full, the element is returned back as an error.
        #[doc = docstring_exclude_vrb!($storage,
            ///
            /// # Examples
            ///
            /// ```
            #[doc = doctest_import!($module, "{PushError, RingBuffer}")]
            ///
            #[doc = doctest_create_ring_buffer!(N = $N, arc = $arc, capacity = 1)]
            ///
            /// assert_eq!(p.push(10), Ok(()));
            /// assert_eq!(p.push(20), Err(PushError::Full(20)));
            /// ```
        )]
        pub fn push(&mut self, value: T) -> Result<(), PushError<T>> {
            if let Some(tail) = self.next_tail() {
                let b = &self.buffer;
                // SAFETY: tail points to an empty slot.
                unsafe { b.slot_ptr(tail).write(value) };
                let tail = b.increment1(tail);
                b.tail.store(tail, Ordering::Release);
                self.cached_tail.set(tail);
                Ok(())
            } else {
                Err(PushError::Full(value))
            }
        }
    };
}

#[rustfmt::skip] // https://github.com/rust-lang/rustfmt/issues/5974
macro_rules! fn_producer_slots {
    (N = $N:ident, arc = $arc:ident, bip = $bip:ident, module = $module:literal) => {
        /// Returns the number of slots available for writing.
        ///
        /// Since items can be concurrently consumed on another thread, the actual number
        /// of available slots may increase at any time
        /// (up to the [`capacity()`](Producer::capacity)).
        ///
        /// To check for a single available slot,
        /// using [`is_full()`](Producer::is_full) is often quicker
        /// (because it might not have to check an atomic variable).
        ///
        #[doc = choice!($bip,
            /// Due to wrap-around of the internal buffer, the reported slots might not be
            /// on a contiguous segment and therefore not entirely available for writing with
            /// [`write_chunk()`](Producer::write_chunk) or
            /// [`write_chunk_uninit()`](Producer::write_chunk_uninit).
            /// To get the number of contiguous slots,
            /// [`slots_contiguous_first()`](Producer::slots_contiguous_first) or
            /// [`slots_contiguous_max()`](Producer::slots_contiguous_max) can be used.
            ::
        )]
        ///
        /// # Examples
        ///
        /// ```
        #[doc = doctest_import!($module, "RingBuffer")]
        ///
        #[doc = doctest_create_ring_buffer!(N = $N, arc = $arc, capacity = 4096)]
        /// assert_eq!(p.push(0.5f32), Ok(()));
        ///
        /// assert_eq!(p.slots(), 4095);
        /// ```
        pub fn slots(&self) -> usize {
            let b = &self.buffer;
            let head = b.head.load(Ordering::Acquire);
            self.cached_head.set(head);
            b.capacity() - b.distance(head, self.cached_tail.get())
        }
    };
}

macro_rules! fn_producer_slots_contiguousX {
    (bip = yes) => {
        /// Returns size of first contiguous chunk and
        /// `true` if `head` has already been refreshed and
        /// `true` if there may be another chunk at the beginning of the buffer.
        // TODO: inline?
        fn slots_contiguous_helper(&self) -> (usize, bool, bool) {
            // TODO: code reuse with write_chunk_uninit() and next_tail()
            let b = &self.buffer;
            let mut head = self.cached_head.get();
            let tail = self.cached_tail.get();
            let is_empty = head == tail;
            let mut collapsed_head = b.collapse_position(head);
            let collapsed_tail = b.collapse_position(tail);
            if is_empty || collapsed_head < collapsed_tail {
                let slots = b.capacity() - collapsed_tail;
                debug_assert!(slots != 0 || b.capacity() == 0);
                return (slots, false, true);
            }
            head = b.head.load(Ordering::Acquire);
            self.cached_head.set(head);
            debug_assert_ne!(head, tail); // buffer is not empty
            collapsed_head = b.collapse_position(head);
            if collapsed_head < collapsed_tail {
                let slots = b.capacity() - collapsed_tail;
                debug_assert!(slots != 0 || b.capacity() == 0);
                return (slots, true, true);
            }
            (collapsed_head - collapsed_tail, true, false)
        }

        /// Returns the number of slots of the next two contiguous segments available
        /// for writing with [`write_chunk()`](Producer::write_chunk) or
        /// [`write_chunk_uninit()`](Producer::write_chunk_uninit).
        ///
        /// When a chunk larger than the first segment is written
        /// (assuming the second number is large enough to allow that),
        /// the first segment is skipped,
        /// effectively reducing the maximum available [`Consumer::slots()`]
        /// (until the segment is overwritten again at a later point).
        ///
        /// In many cases, the second number will be `0`, but whenever the internal buffer
        /// wraps around, two contiguous segments might be available for writing.
        /// If the first number is `0`, the second will be `0` as well.
        ///
        /// If you are only interested in the first number, using
        /// [`slots_contiguous_first()`](Producer::slots_contiguous_first)
        /// should be slightly more efficient.
        ///
        /// The sum of both numbers is returned by [`slots()`](Producer::slots), their maximum
        /// is returned by [`slots_contiguous_max()`](Producer::slots_contiguous_max).
        ///
        /// Since items can be concurrently consumed on another thread, the actual number
        /// of available slots may increase at any time
        /// (up to the [`capacity()`](Producer::capacity)).
        // TODO: inline?
        pub fn slots_contiguous(&self) -> (usize, usize) {
            let (slots, refreshed, try_again) = self.slots_contiguous_helper();
            if !try_again {
                return (slots, 0);
            }
            let mut head = self.cached_head.get();
            let b = &self.buffer;
            if !refreshed {
                head = b.head.load(Ordering::Acquire);
                self.cached_head.set(head);
            }
            (slots, b.collapse_position(head))
        }

        /// Returns the number of slots of the next contiguous segment available for writing with
        /// [`write_chunk()`](Producer::write_chunk) or
        /// [`write_chunk_uninit()`](Producer::write_chunk_uninit).
        ///
        /// This is the same as the first number returned by
        /// [`slots_contiguous()`](Producer::slots_contiguous), but slightly more efficient.
        ///
        /// If you want to avoid skipping any slots, you should use this method instead of
        /// [`slots_contiguous_max()`](Producer::slots_contiguous_max).
        // TODO: inline?
        pub fn slots_contiguous_first(&self) -> usize {
            self.slots_contiguous_helper().0
        }

        /// Convenience function to obtain the maximum of the two values returned by
        /// [`slots_contiguous()`](Producer::slots_contiguous).
        ///
        /// This number will also be reported via a [`ChunkError`] when calling
        /// [`write_chunk()`](Producer::write_chunk) or
        /// [`write_chunk_uninit()`](Producer::write_chunk_uninit) with a larger number.
        ///
        /// If you want to avoid skipping any slots, you should use
        /// [`slots_contiguous_first()`](Producer::slots_contiguous_first) instead.
        // TODO: inline?
        pub fn slots_contiguous_max(&self) -> usize {
            let (one, two) = self.slots_contiguous();
            one.max(two)
        }
    };
    (bip = no) => {};
}

#[rustfmt::skip] // https://github.com/rust-lang/rustfmt/issues/5974
macro_rules! fn_producer_is_full {
    (storage = $storage:ident, N = $N:ident, arc = $arc:ident, bip = $bip:ident, module = $module:literal) => {
        /// Returns `true` if there are currently no slots available for writing.
        ///
        /// A full ring buffer might cease to be full at any time
        /// if the corresponding [`Consumer`] is consuming items in another thread.
        #[doc = docstring_exclude_vrb!($storage,
            ///
            /// # Examples
            ///
            /// ```
            #[doc = doctest_import!($module, "RingBuffer")]
            ///
            #[doc = doctest_create_ring_buffer!(N = $N, arc = $arc, capacity = 1)]
            ///
            /// assert!(!p.is_full());
            /// assert_eq!(p.push(10), Ok(()));
            /// assert!(p.is_full());
            /// ```
            ///
            /// Since items can be concurrently consumed on another thread, the ring buffer
            /// might not be full for long:
            ///
            /// ```
            #[doc = doctest_import!($module, "RingBuffer", "# ")]
            #[doc = doctest_create_ring_buffer!(N = $N, arc = $arc, capacity = 1, "# ")]
            /// # assert_eq!(p.push(10), Ok(()));
            /// if p.is_full() {
            ///     // The buffer might be full, but it might as well not be
            ///     // if an item was just consumed on another thread.
            /// }
            /// ```
            ///
            /// However, if it's not full, another thread cannot change that:
            ///
            /// ```
            #[doc = doctest_import!($module, "RingBuffer", "# ")]
            #[doc = doctest_create_ring_buffer!(N = $N, arc = $arc, capacity = 1, "# ")]
            /// # assert_eq!(p.push(10), Ok(()));
            /// if !p.is_full() {
            ///     // At least one slot is guaranteed to be available for writing.
            /// }
            /// ```
            ///
            #[doc = choice!($bip,
                /// A ring buffer (of the Bip Buffer variety) can be full even if it contains
                /// fewer items than its [`capacity()`](Producer::capacity()).
                ///
                /// ```
                /// use std::io::{Read, Write};
                #[doc = doctest_import!($module, "RingBuffer")]
                ///
                #[doc = doctest_create_ring_buffer!(N = $N, arc = $arc, capacity = 8)]
                /// assert_eq!(p.write(&[1, 2, 3, 4, 5]).unwrap(), 5);
                /// let mut a = [0; 5];
                /// assert_eq!(c.read(&mut a).unwrap(), 5);
                /// assert_eq!(a, [1, 2, 3, 4, 5]);
                /// // This will skip 3 slots:
                /// assert_eq!(p.write(&[5, 4, 3, 2, 1]).unwrap(), 5);
                /// assert!(p.is_full());
                /// assert_eq!(p.capacity(), 8);
                /// assert_eq!(p.slots(), 0);
                /// assert_eq!(c.slots(), 5);
                /// // TODO: slots() might actually reset "skip" in this case?!?
                /// // TODO: better write 5 then read 4 then write 4?
                /// ```
                ::
            )]
        )]
        pub fn is_full(&self) -> bool {
            self.next_tail().is_none()
        }
    };
}

#[rustfmt::skip] // https://github.com/rust-lang/rustfmt/issues/5974
macro_rules! fn_pc_capacity {
    (N = $N:ident, arc = $arc:ident, bip = $bip:ident, module = $module:literal) => {
        /// Returns the total capacity of the queue.
        ///
        /// At any time, the capacity is subdivided into
        /// [`Producer::slots()`] available for writing and
        /// [`Consumer::slots()`] available for
        #[doc = choice!($bip,
            /// reading, as well as potentially some slots that have been skipped in
            /// [`Producer::write_chunk`] or [`Producer::write_chunk_uninit`].
            ::
            /// reading.
        )]
        ///
        /// # Examples
        ///
        /// ```
        #[doc = doctest_import!($module, "RingBuffer")]
        ///
        #[doc = doctest_create_ring_buffer!(N = $N, arc = $arc, capacity = 4096)]
        ///
        /// assert_eq!(p.push(-0.7), Ok(()));
        /// assert_eq!(p.slots(), 4095);
        /// assert_eq!(c.slots(), 1);
        /// assert_eq!(p.capacity(), 4096);
        /// assert_eq!(c.capacity(), 4096);
        /// ```
        pub fn capacity(&self) -> usize {
            self.buffer.capacity()
        }
    };
}

/// NB: next_tail() can also be used for "bip", because `b.skip` is never set.
/// One element can always be inserted without skipping.
macro_rules! fn_producer_next_tail {
    () => {
        /// Get the tail position for writing the next slot, if available.
        ///
        /// This is a strict subset of the functionality implemented in `write_chunk_uninit()`.
        /// For performance, this special case is implemented separately.
        fn next_tail(&self) -> Option<usize> {
            let head = self.cached_head.get();
            let tail = self.cached_tail.get();
            let b = &self.buffer;
            // Check if the queue is *possibly* full.
            if b.distance(head, tail) == b.capacity() {
                // Refresh the head ...
                let head = b.head.load(Ordering::Acquire);
                self.cached_head.set(head);
                // ... and check if it's *really* full.
                if b.distance(head, tail) == b.capacity() {
                    // `head` didn't change, queue is full.
                    return None;
                }
            }
            Some(tail)
        }
    };
}

macro_rules! fn_consumer_pop {
    (N = $N:ident, arc = $arc:ident, module = $module:literal) => {
        /// Attempts to pop the next element from the queue.
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
        #[doc = doctest_import!($module, "{PopError, RingBuffer}")]
        ///
        #[doc = doctest_create_ring_buffer!(N = $N, arc = $arc, capacity = 1)]
        ///
        /// assert_eq!(p.push(10), Ok(()));
        /// assert_eq!(c.pop(), Ok(10));
        /// assert_eq!(c.pop(), Err(PopError::Empty));
        /// ```
        ///
        /// To obtain an [`Option<T>`](Option), use [`.ok()`](Result::ok) on the result.
        ///
        /// ```
        #[doc = doctest_import!($module, "RingBuffer", "# ")]
        #[doc = doctest_create_ring_buffer!(N = $N, arc = $arc, capacity = 1, "# ")]
        /// assert_eq!(p.push(20), Ok(()));
        /// assert_eq!(c.pop().ok(), Some(20));
        /// ```
        pub fn pop(&mut self) -> Result<T, PopError> {
            if let Some(head) = self.next_head() {
                let b = &self.buffer;
                // SAFETY: head points to an initialized slot.
                let value = unsafe { b.slot_ptr(head).read() };
                let head = b.increment1(head);
                b.head.store(head, Ordering::Release);
                self.cached_head.set(head);
                Ok(value)
            } else {
                Err(PopError::Empty)
            }
        }
    };
}

macro_rules! fn_consumer_peek {
    (N = $N:ident, arc = $arc:ident, module = $module:literal) => {
        /// Attempts to get read access to the next element in the queue without removing it.
        ///
        /// # Errors
        ///
        /// If the queue is empty, an error is returned.
        ///
        /// # Examples
        ///
        /// ```
        #[doc = doctest_import!($module, "{PeekError, RingBuffer}")]
        ///
        #[doc = doctest_create_ring_buffer!(N = $N, arc = $arc, capacity = 1)]
        ///
        /// assert_eq!(c.peek(), Err(PeekError::Empty));
        /// assert_eq!(p.push(10), Ok(()));
        /// assert_eq!(c.peek(), Ok(&10));
        /// assert_eq!(c.peek(), Ok(&10));
        /// ```
        pub fn peek(&self) -> Result<&T, PeekError> {
            if let Some(head) = self.next_head() {
                // SAFETY: head points to an initialized slot.
                Ok(unsafe { &*self.buffer.slot_ptr(head) })
            } else {
                Err(PeekError::Empty)
            }
        }
    };
}

macro_rules! fn_consumer_slots_docstring {
    (N = $N:ident, arc = $arc:ident, bip = $bip:ident, module = $module:literal) => { docstring!(
        /// Returns the number of slots available for reading.
        ///
        #[doc = choice!($bip,
            ::
            /// This number will also be reported via a [`ChunkError`] when calling
            /// [`read_chunk()`](Consumer::read_chunk) with a larger number.
        )]
        ///
        /// Since items can be concurrently produced on another thread, the actual number
        /// of available slots may increase at any time
        /// (up to the [`capacity()`](Consumer::capacity)).
        ///
        /// To check for a single available slot,
        /// using [`is_empty()`](Consumer::is_empty) is often quicker
        /// (because it might not have to check an atomic variable).
        ///
        #[doc = choice!($bip,
            /// Due to wrap-around of the internal buffer, the reported slots might not be
            /// on a contiguous segment and therefore not entirely available for reading with
            /// [`read_chunk()`](Consumer::read_chunk). To get the number of contiguous slots,
            /// [`slots_contiguous_first()`](Consumer::slots_contiguous_first) can be used.
            ::
        )]
        ///
        /// # Examples
        ///
        /// ```
        #[doc = doctest_import!($module, "RingBuffer")]
        ///
        #[doc = doctest_create_ring_buffer!(N = $N, arc = $arc, capacity = 1024)]
        ///
        /// assert_eq!(c.slots(), 0);
        /// assert_eq!(p.push(0.0), Ok(()));
        /// assert_eq!(c.slots(), 1);
        /// ```
    )};
}

macro_rules! fn_consumer_slots {
    (N = $N:ident, arc = $arc:ident, bip = yes, module = $module:literal) => {
        #[doc = fn_consumer_slots_docstring!(N = $N, arc = $arc, bip = yes, module = $module)]
        // TODO: code reuse with other "slots" variations?
        pub fn slots(&self) -> usize {
            let b = &self.buffer;
            let mut head = self.cached_head.get();
            let tail = b.tail.load(Ordering::Acquire);
            self.cached_tail.set(tail);
            if head == tail {
                return 0;
            }
            let collapsed_head = b.collapse_position(head);
            let collapsed_tail = b.collapse_position(tail);
            if collapsed_head < collapsed_tail {
                collapsed_tail - collapsed_head
            } else {
                let skip = b.skip.load(Ordering::Acquire);
                // TODO: code reuse with slots_contiguous..()?
                let slots_at_end = skip - collapsed_head;
                if slots_at_end != 0 {
                    return collapsed_tail + slots_at_end;
                }
                // There are no more slots at the end of the buffer,
                // let's clear `skip` and wrap around!
                if skip != b.capacity() {
                    // NB: `skip` is stored before `head`.
                    b.skip.store(b.capacity(), Ordering::Release);
                }
                head = b.increment(head, b.capacity() - collapsed_head);
                b.head.store(head, Ordering::Release);
                self.cached_head.set(head);
                debug_assert_eq!(b.collapse_position(head), 0);
                collapsed_tail
            }
        }
    };
    (N = $N:ident, arc = $arc:ident, bip = no, module = $module:literal) => {
        #[doc = fn_consumer_slots_docstring!(N = $N, arc = $arc, bip = no, module = $module)]
        pub fn slots(&self) -> usize {
            let b = &self.buffer;
            let tail = b.tail.load(Ordering::Acquire);
            self.cached_tail.set(tail);
            b.distance(self.cached_head.get(), tail)
        }
    };
}

macro_rules! fn_consumer_slots_contiguousX {
    (bip = yes) => {
        fn slots_contiguous_helper(&self) -> (usize, bool) {
            // TODO: code reuse with read_chunk() and next_head()?
            let b = &self.buffer;
            let mut head = self.cached_head.get();
            let mut tail = self.cached_tail.get();
            let mut slots = 0;
            let mut is_empty = head == tail;
            let mut collapsed_head = b.collapse_position(head);
            let mut collapsed_tail = b.collapse_position(tail);
            if !is_empty && collapsed_tail <= collapsed_head {
                // NB: We are only allowed to use `skip` if (collapsed) `tail < head`
                //     (or if the buffer is full).
                let skip = b.skip.load(Ordering::Acquire);
                slots = skip - collapsed_head;
                if slots != 0 {
                    return (slots, false);
                }
                // There are no more slots at the end of the buffer,
                // let's clear `skip` and wrap around!
                if skip != b.capacity() {
                    b.skip.store(b.capacity(), Ordering::Release);
                }
                head = b.increment(head, b.capacity() - collapsed_head);
                // NB: `skip` is stored before `head`.
                b.head.store(head, Ordering::Release);
                self.cached_head.set(head);
                collapsed_head = b.collapse_position(head);
            } else {
                // nothing to do here, we have to refresh tail
            }
            tail = b.tail.load(Ordering::Acquire);
            self.cached_tail.set(tail);
            collapsed_tail = b.collapse_position(tail);
            is_empty = head == tail;
            if is_empty || collapsed_head < collapsed_tail {
                debug_assert_eq!(slots, 0);
                (collapsed_tail - collapsed_head, true)
            } else {
                // TODO: if tail has wrapped around, there might be slots between head and skip!
                // TODO: repeat the code from above?
                // TODO: another `bool` might be needed ...
                (collapsed_tail, true)
            }
        }

        /// Returns the number of slots of the next two contiguous segments available
        /// for reading with [`read_chunk()`](Consumer::read_chunk).
        ///
        /// In many cases, the second number will be `0`, but whenever the internal buffer
        /// wraps around, two contiguous segments might be available for reading.
        /// If the first number is `0`, the second will be `0` as well.
        ///
        /// The sum of both numbers is returned by [`slots()`](Consumer::slots).
        ///
        /// If you are only interested in the first number, using
        /// [`slots_contiguous_first()`](Consumer::slots_contiguous_first)
        /// should be slightly more efficient.
        ///
        /// The first number will also be reported via a [`ChunkError`] when calling
        /// [`read_chunk()`](Consumer::read_chunk) with a larger number.
        ///
        /// Since items can be concurrently produced on another thread, the actual number
        /// of available slots may increase at any time
        /// (up to a certain maximum that depends on the current read index and
        /// whether and how many slots have been skipped using
        /// [`Producer::write_chunk()`] or [`Producer::write_chunk_uninit()`],
        /// but at most up to the [`capacity()`](Consumer::capacity)).
        pub fn slots_contiguous(&self) -> (usize, usize) {
            let (slots, refreshed) = self.slots_contiguous_helper();
            if refreshed {
                return (slots, 0);
            }
            let b = &self.buffer;
            let head = self.cached_head.get();
            let collapsed_head = b.collapse_position(head);
            let tail = b.tail.load(Ordering::Acquire);
            self.cached_tail.set(tail);
            let collapsed_tail = b.collapse_position(tail);
            let is_empty = head == tail;
            if is_empty || collapsed_head < collapsed_tail {
                debug_assert_eq!(slots, 0);
                (collapsed_tail - collapsed_head, 0)
            } else {
                (slots, collapsed_tail)
            }
        }

        /// Returns the number of slots of the next contiguous segment available for reading with
        /// [`read_chunk()`](Consumer::read_chunk).
        ///
        /// This is the same as the first number returned by
        /// [`slots_contiguous()`](Consumer::slots_contiguous), but slightly more efficient.
        pub fn slots_contiguous_first(&self) -> usize {
            self.slots_contiguous_helper().0
        }
    };
    (bip = no) => {};
}

macro_rules! fn_consumer_is_empty {
    (N = $N:ident, arc = $arc:ident, module = $module:literal) => {
        /// Returns `true` if there are currently no slots available for reading.
        ///
        /// An empty ring buffer might cease to be empty at any time
        /// if the corresponding [`Producer`] is producing items in another thread.
        ///
        /// # Examples
        ///
        /// ```
        #[doc = doctest_import!($module, "RingBuffer")]
        ///
        #[doc = doctest_create_ring_buffer!(N = $N, arc = $arc, capacity = 1)]
        ///
        /// assert!(c.is_empty());
        /// assert_eq!(p.push(0.0), Ok(()));
        /// assert!(!c.is_empty());
        /// ```
        ///
        /// Since items can be concurrently produced on another thread, the ring buffer
        /// might not be empty for long:
        ///
        /// ```
        #[doc = doctest_import!($module, "RingBuffer", "# ")]
        #[doc = doctest_create_ring_buffer!(N = $N, arc = $arc, capacity = 1, "# ")]
        /// # assert_eq!(p.push(0.0), Ok(()));
        /// if c.is_empty() {
        ///     // The buffer might be empty, but it might as well not be
        ///     // if an item was just produced on another thread.
        /// }
        /// ```
        ///
        /// However, if it's not empty, another thread cannot change that:
        ///
        /// ```
        #[doc = doctest_import!($module, "RingBuffer", "# ")]
        #[doc = doctest_create_ring_buffer!(N = $N, arc = $arc, capacity = 1, "# ")]
        /// # assert_eq!(p.push(0.0), Ok(()));
        /// if !c.is_empty() {
        ///     // At least one slot is guaranteed to be available for reading.
        /// }
        /// ```
        pub fn is_empty(&self) -> bool {
            self.next_head().is_none()
        }
    };
}

macro_rules! fn_producer_is_abandoned {
    (N = $N:ident, arc = yes, module = $module:literal) => {
        /// Returns `true` if the corresponding [`Consumer`] has been destroyed.
        ///
        /// TODO: update this note:
        ///
        /// Note that since Rust version 1.74.0, this is not synchronizing with the consumer thread
        /// anymore, see <https://github.com/mgeier/rtrb/issues/114>.
        /// In a future version of `rtrb`, the synchronizing behavior might be restored.
        ///
        /// # Examples
        ///
        /// ```
        #[doc = doctest_import!($module, "RingBuffer")]
        ///
        #[doc = doctest_create_ring_buffer!(N = $N, arc = yes, capacity = 7)]
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
        #[doc = doctest_import!($module, "RingBuffer", "# ")]
        #[doc = doctest_create_ring_buffer!(N = $N, arc = yes, capacity = 1, "# ")]
        /// # assert_eq!(p.push(10), Ok(()));
        /// if !p.is_abandoned() {
        ///     // Right now, the consumer might still be alive, but it might as well not be
        ///     // if another thread has just dropped it.
        /// }
        /// ```
        ///
        /// However, if it already is abandoned, it will stay that way:
        ///
        /// ```
        #[doc = doctest_import!($module, "RingBuffer", "# ")]
        #[doc = doctest_create_ring_buffer!(N = $N, arc = yes, capacity = 1, "# ")]
        /// # assert_eq!(p.push(10), Ok(()));
        /// if p.is_abandoned() {
        ///     // The consumer does definitely not exist anymore.
        /// }
        /// ```
        pub fn is_abandoned(&self) -> bool {
            self.buffer.flags.load(Ordering::Acquire) & IS_ABANDONED != 0
        }
    };
    (N = $N:ident, arc = no, module = $module:literal) => {};
}

macro_rules! fn_consumer_is_abandoned {
    (N = $N:ident, arc = yes, module = $module:literal) => {
        /// Returns `true` if the corresponding [`Producer`] has been destroyed.
        ///
        /// TODO: update this note:
        ///
        /// Note that since Rust version 1.74.0, this is not synchronizing with the producer thread
        /// anymore, see <https://github.com/mgeier/rtrb/issues/114>.
        /// In a future version of `rtrb`, the synchronizing behavior might be restored.
        ///
        /// # Examples
        ///
        /// ```
        #[doc = doctest_import!($module, "RingBuffer")]
        ///
        #[doc = doctest_create_ring_buffer!(N = $N, arc = yes, capacity = 7)]
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
        #[doc = doctest_import!($module, "RingBuffer", "# ")]
        #[doc = doctest_create_ring_buffer!(N = $N, arc = yes, capacity = 1, "# ")]
        /// # assert_eq!(p.push(10), Ok(()));
        /// if !c.is_abandoned() {
        ///     // Right now, the producer might still be alive, but it might as well not be
        ///     // if another thread has just dropped it.
        /// }
        /// ```
        ///
        /// However, if it already is abandoned, it will stay that way:
        ///
        /// ```
        #[doc = doctest_import!($module, "RingBuffer", "# ")]
        #[doc = doctest_create_ring_buffer!(N = $N, arc = yes, capacity = 1, "# ")]
        /// # assert_eq!(p.push(10), Ok(()));
        /// if c.is_abandoned() {
        ///     // The producer does definitely not exist anymore.
        /// }
        /// ```
        pub fn is_abandoned(&self) -> bool {
            self.buffer.flags.load(Ordering::Acquire) & IS_ABANDONED != 0
        }
    };
    (N = $N:ident, arc = no, module = $module:literal) => {};
}

macro_rules! fn_producer_has_consumer {
    (N = $N:ident, arc = yes, module = $module:literal) => {};
    (N = $N:ident, arc = no, module = $module:literal) => {
        /// Returns `true` if the [`RingBuffer`] connected to this `Producer`
        /// is also connected to a [`Consumer`].
        ///
        /// This can change at any time when another thread creates or drops a `Consumer`.
        ///
        /// See also [`RingBuffer::has_consumer()`].
        pub fn has_consumer(&self) -> bool {
            self.buffer.flags.load(Ordering::SeqCst) & HAS_CONSUMER != 0
        }
    };
}

macro_rules! fn_consumer_has_producer {
    (N = $N:ident, arc = yes, module = $module:literal) => {};
    (N = $N:ident, arc = no, module = $module:literal) => {
        /// Returns `true` if the [`RingBuffer`] connected to this `Consumer`
        /// is also connected to a [`Producer`].
        ///
        /// This can change at any time when another thread creates or drops a `Producer`.
        ///
        /// See also [`RingBuffer::has_producer()`].
        pub fn has_producer(&self) -> bool {
            self.buffer.flags.load(Ordering::SeqCst) & HAS_PRODUCER != 0
        }
    };
}

/// Get the `head` position for reading the next slot, if available.
///
/// This is a strict subset of the functionality implemented in `read_chunk()`.
/// For performance, this special case is implemented separately.
macro_rules! fn_consumer_next_head {
    (bip = yes) => {
        fn next_head(&self) -> Option<usize> {
            // NB: cached_head is always up-to-date, no need for atomic load here.
            let mut head = self.cached_head.get();
            let mut tail = self.cached_tail.get();
            let b = &self.buffer;

            // Check if the queue is *possibly* empty.
            if head == tail {
                // Refresh the tail ...
                tail = b.tail.load(Ordering::Acquire);
                self.cached_tail.set(tail);
                // ... and check if it's *really* empty.
                if head == tail {
                    // `tail` didn't change, queue is empty.
                    return None;
                } else if b.collapse_position(head) < b.collapse_position(tail) {
                    // `tail` did change, but it didn't wrap around.
                    return Some(head);
                }
            } else if head < tail {
                // The tail might have wrapped around in the meantime.
                tail = b.tail.load(Ordering::Acquire);
                self.cached_tail.set(tail);
            } else {
                // The tail cannot overtake the head, no need to refresh at this point.
            }
            debug_assert_ne!(head, tail);
            if b.collapse_position(tail) < b.collapse_position(head) {
                // NB: We are only allowed to use `skip` if (collapsed) `tail < head`.
                let skip = b.skip.load(Ordering::Acquire);
                if b.collapse_position(head) == skip {
                    // Nothing to read at the end of the buffer, wrap `head` and clear `skip`.
                    head = b.increment(head, b.capacity() - skip);
                    b.head.store(head, Ordering::Release);
                    self.cached_head.set(head);
                    b.skip.store(b.capacity(), Ordering::Release);

                    // NB: The producer only sets `skip` if it writes at least one slot
                    // at the beginning of the buffer.  Therefore, we know that the
                    // wrapped-around `head` is valid for reading (at least) one slot.
                }
            }
            Some(head)
        }
    };
    (bip = no) => {
        fn next_head(&self) -> Option<usize> {
            let head = self.cached_head.get();
            let tail = self.cached_tail.get();

            // Check if the queue is *possibly* empty.
            if head == tail {
                // Refresh the tail ...
                let tail = self.buffer.tail.load(Ordering::Acquire);
                self.cached_tail.set(tail);
                // ... and check if it's *really* empty.
                if head == tail {
                    // `tail` didn't change, queue is empty.
                    return None;
                }
            }
            Some(head)
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

macro_rules! fn_read_chunk_uninit_commit_unchecked {
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
            let mut head = self.cached_head.get();
            let tail = self.cached_tail.get();
            // TODO: check if everything is compatible with power-of-2 addressing.
            let mut slots = 0;
            let mut head_has_been_refreshed = false;
            // Collapsing the indices makes it impossible to distinguish empty and full,
            // so we check for emptiness before collapsing.
            let is_empty = head == tail;
            let mut collapsed_head = b.collapse_position(head);
            let collapsed_tail = b.collapse_position(tail);
            if !is_empty && collapsed_tail <= collapsed_head {
                // Is there enough space between `tail` and `head`?
                slots = collapsed_head - collapsed_tail;
                if slots < n {
                    // Refresh head ...
                    head = b.head.load(Ordering::Acquire);
                    self.cached_head.set(head);
                    collapsed_head = b.collapse_position(head);
                    head_has_been_refreshed = true;
                    // ... and try again.
                    let is_empty = head == tail;
                    if !is_empty && collapsed_tail <= collapsed_head {
                        // `head` did not wrap around after refreshing.
                        // However, we also need to check if it landed on `skip`:
                        let skip = b.skip.load(Ordering::Acquire);
                        if collapsed_head == skip {
                            // `head` is beyond the valid slots and has to be reset.
                            head = b.increment(head, self.capacity() - skip);
                            b.head.store(head, Ordering::Release);
                            // TODO: order of storing head and skip?
                            self.cached_head.set(head);
                            collapsed_head = b.collapse_position(head);
                            b.skip.store(b.capacity(), Ordering::Release);
                            debug_assert_eq!(collapsed_head, 0);
                            // `head` did wrap around after all, we'll continue below.
                        } else {
                            slots = collapsed_head - collapsed_tail;
                            if slots < n {
                                return Err(ChunkError::TooFewSlots(slots));
                            }
                        }
                    } else {
                        // `head` did wrap around, we'll continue below.
                    }
                }
            } else {
                // No need to refresh `head` here, it cannot overtake `tail`.
            }
            let offset;
            if slots < n {
                // NB: If we reach this point, we know that either the buffer is empty,
                // or collapsed_head < collapsed_tail.
                // Is there enough space at the end of the buffer?
                slots = b.capacity() - collapsed_tail;
                if slots < n {
                    // Nope, let's check the beginning.

                    // TODO: interaction/reuse with slots() et al.?

                    slots = slots.max(collapsed_head);
                    if slots < n {
                        // TODO: check if this early return/local variable is an actual optimization?
                        if head_has_been_refreshed {
                            return Err(ChunkError::TooFewSlots(slots));
                        }
                        head = b.head.load(Ordering::Acquire);
                        self.cached_head.set(head);
                        collapsed_head = b.collapse_position(head);
                        slots = slots.max(collapsed_head);
                        if slots < n {
                            return Err(ChunkError::TooFewSlots(slots));
                        }
                    }
                    // NB: `tail` will be (conditionally) reset in `commit_unchecked()`.
                    offset = 0;
                } else {
                    offset = collapsed_tail;
                }
            } else {
                offset = collapsed_tail;
            }
            // SAFETY: `offset` has been set to a valid position.
            Ok(unsafe { WriteChunkUninit::new(self, n, offset) })
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
    (N = $N:ident, bip = yes, contiguous = $contiguous:ident) => {
        #[doc = fn_consumer_read_chunk_docstring!(bip = yes, contiguous = $contiguous)]
        pub fn read_chunk(
            &mut self,
            n: usize,
        ) -> Result<generic!(ReadChunk<'_>, N = $N), ChunkError> {
            let b = &self.buffer;
            let mut head = self.cached_head.get();
            let mut tail = self.cached_tail.get();
            let mut slots = 0;
            let mut tail_has_been_refreshed = false;
            // Collapsing the indices makes it impossible to distinguish empty and full,
            // so we check for emptiness before collapsing.
            let is_empty = head == tail;
            let mut collapsed_head = b.collapse_position(head);
            let mut collapsed_tail = b.collapse_position(tail);
            if is_empty || collapsed_head < collapsed_tail {
                slots = collapsed_tail - collapsed_head;
                if slots < n {
                    // Refresh the tail ...
                    tail = b.tail.load(Ordering::Acquire);
                    tail_has_been_refreshed = true;
                    self.cached_tail.set(tail);
                    collapsed_tail = b.collapse_position(tail);
                    // ... and check again.
                    let is_empty = head == tail;
                    if is_empty || collapsed_head < collapsed_tail {
                        // `tail` did not wrap around.
                        slots = collapsed_tail - collapsed_head;
                        if slots < n {
                            return Err(ChunkError::TooFewSlots(slots));
                        }
                    } else {
                        // `tail` did wrap around, we'll continue below.
                    }
                }
            } else {
                // No need to refresh `tail`, it cannot overtake `head`.
            }
            if slots < n {
                // NB: We are only allowed to use `skip` if (collapsed) `tail < head`
                //     (or if the buffer is full).
                let skip = b.skip.load(Ordering::Acquire);
                slots = skip - collapsed_head;
                if slots == 0 {
                    // No more slots at the end of the buffer, let's wrap around.
                    if skip != b.capacity() {
                        b.skip.store(b.capacity(), Ordering::Release);
                    }
                    head = b.increment(head, b.capacity() - collapsed_head);
                    // NB: `skip` is stored before `head`.
                    b.head.store(head, Ordering::Release);
                    self.cached_head.set(head);
                    collapsed_head = b.collapse_position(head);
                    slots = collapsed_tail - collapsed_head;
                    if slots < n {
                        if tail_has_been_refreshed {
                            return Err(ChunkError::TooFewSlots(slots));
                        }
                        tail = b.tail.load(Ordering::Acquire);
                        self.cached_tail.set(tail);
                        collapsed_tail = b.collapse_position(tail);
                        slots = collapsed_tail - collapsed_head;
                    }
                }
                if slots < n {
                    return Err(ChunkError::TooFewSlots(slots));
                }
            }
            let offset = collapsed_head;
            // SAFETY: `offset` has been set to a valid position.
            Ok(unsafe { ReadChunk::new(self, n, offset) })
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

macro_rules! impl_send_for_producer {
    (N = $N:ident, arc = $arc:ident, module = $module:literal) => {
        // SAFETY: After moving a producer to another thread, there is still only a single thread
        // that can access the producer side of the queue.
        impl_!(
            /// It can be moved ...
            /// ```
            #[doc = doctest_import!($module, "Producer")]
            /// fn assert_send<X: Send>() {}
            #[doc = concat!("assert_send::<", doctest_ty!(Producer, u8, 8, N = $N), ">();")]
            /// ```
            /// ... but not shared between threads:
            /// ```compile_fail
            #[doc = doctest_import!($module, "Producer", "# ")]
            /// fn assert_sync<X: Sync>() {}
            #[doc = concat!("assert_sync::<", doctest_ty!(Producer, u8, 8, N = $N), ">();")]
            /// ```
            Producer, trait unsafe = Send, N = $N, arc = $arc,
            where
                T: Send,
                generic!(RingBuffer, N = $N): Sync
            {});
    };
}

macro_rules! impl_send_for_consumer {
    (N = $N:ident, arc = $arc:ident, module = $module:literal) => {
        // SAFETY: After moving a consumer to another thread, there is still only a single thread
        // that can access the consumer side of the queue.
        impl_!(
            /// It can be moved ...
            /// ```
            #[doc = doctest_import!($module, "Consumer")]
            /// fn assert_send<X: Send>() {}
            #[doc = concat!("assert_send::<", doctest_ty!(Consumer, u8, 8, N = $N), ">();")]
            /// ```
            /// ... but not shared between threads:
            /// ```compile_fail
            #[doc = doctest_import!($module, "Consumer", "# ")]
            /// fn assert_sync<X: Sync>() {}
            #[doc = concat!("assert_sync::<", doctest_ty!(Consumer, u8, 8, N = $N), ">();")]
            /// ```
            Consumer, trait unsafe = Send, N = $N, arc = $arc,
            where
                T: Send,
                generic!(RingBuffer, N = $N): Sync
            {});
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
    (storage = $storage:ident, N = $N:ident, arc = $arc:ident, contiguous = $contiguous:ident, module = $module:literal) => { docstring!(
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
    #[doc = choice!($contiguous,
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
    #[doc = choice!($contiguous,
        /// // NB: Those free slots are available in two contiguous chunks:
        /// assert_eq!(p.slots_contiguous(), (1, 2));
        ::
    )]
    ///
    /// let data = vec![20, 21];
    /// // NB: write_chunk_uninit() could be used for possibly better performance:
    /// if let Ok(mut chunk) = p.write_chunk(2) {
    #[doc = choice!($contiguous,
        ///     // NB: a chunk of 2 was not available at the end of the buffer,
        ///     //     so one slot was skipped and we got a chunk at the beginning.
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
    #[doc = choice!($contiguous,
        /// assert!(p.is_full()); // The skipped slot is not available (for now)!
        /// assert_eq!(c.pop(), Ok(12));
        /// // TODO: try also with read_chunk(1) (but then remove?)
        /// //c.read_chunk(1).unwrap().commit_all();
        /// // Popping this element has unblocked the skipped slot:
        /// // TODO: fix this:
        /// //assert_eq!(p.slots(), 2);
        /// //assert_eq!(p.slots_contiguous(), (2, 0));
        /// // TODO: try also this alternative (but then remove it?):
        /// assert!(p.write_chunk(2).is_ok());
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
    #[doc = choice!($contiguous,
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
