//!
//! Phil Howard is maybe the inventor (2001?):
//! http://web.archive.org/web/20190208212054/http://freshmeat.sourceforge.net/projects/vrb/
//!
//! http://web.archive.org/web/20140705114711/http://vrb.sourceforge.net/
//!
//! code available here (as part of LIBH): https://web.archive.org/web/20140625200016/http://libh.slashusr.org/
//!
//! Potential Windows solution:
//! https://fgiesen.wordpress.com/2012/07/21/the-magic-ring-buffer/

use core::{marker::PhantomData, mem, sync::atomic::AtomicU8};

use crate::{
    chunks::ChunkError,
    diy::{Calc, IndexCalculation, Indices, Ptr, Storage},
    CachePaddedIndices, PopError, PushError,
};

// TODO: move MmapStorage to "diy" module?

#[derive(Debug)]
pub struct MmapStorage<T, const C: u8, I: Indices> {
    indices: I,

    flags: AtomicU8,

    /// Pointer to the first mapped region
    data_ptr: *mut T,

    capacity: usize,

    /// Indicates that dropping a `MmapStorage` may drop elements of type `T`.
    _marker: PhantomData<T>,
}

/// `T` is not `Sync` because we never share it across threads.
// SAFETY: There is no mutable state (except for interior mutability).
unsafe impl<T: Send, const C: u8, I: Indices + Sync> Sync for MmapStorage<T, C, I> {}

// NB: MmapStorage doesn't need to be `Send` because it is never moved.

// Any `Calc` should work, but the capacity will always be a power of two
// (a multiple of (page size / size of `T`)),
// so `PowerOfTwo` probably makes most sense.
impl<T, const C: u8, I: Indices> MmapStorage<T, C, I> {
    #[allow(clippy::new_ret_no_self, clippy::type_complexity)]
    pub fn new(
        capacity: usize,
    ) -> (
        crate::diy::Producer<Ptr<MmapStorage<T, C, I>>>,
        crate::diy::Consumer<Ptr<MmapStorage<T, C, I>>>,
    ) {
        const {
            // NB: This also disallows zero-sized types:
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
        assert_eq!(capacity, Calc::from_u8(C).update_capacity(capacity));
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
        Ptr::new(Self {
            indices: I::INIT,
            flags: AtomicU8::new(0),
            data_ptr,
            capacity,
            _marker: PhantomData,
        })
    }
}

impl<T, const C: u8, I: Indices> Drop for MmapStorage<T, C, I> {
    /// Drops all non-empty slots.
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

impl<T, const C: u8, I: Indices> IndexCalculation for MmapStorage<T, C, I> {
    const CALC: Calc = Calc::from_u8(C);

    fn capacity(&self) -> usize {
        self.capacity
    }
}

// SAFETY: all methods must be implemented correctly, or the whole thing is unsound
unsafe impl<T, const C: u8, I: Indices> Storage for MmapStorage<T, C, I> {
    type Item = T;
    type Indices = I;

    fn indices(&self) -> &Self::Indices {
        &self.indices
    }

    fn flags(&self) -> &AtomicU8 {
        &self.flags
    }

    fn data_ptr(&self) -> *mut Self::Item {
        self.data_ptr
    }
}

impl<T, const C: u8, I: Indices> PartialEq for MmapStorage<T, C, I> {
    fn eq(&self, other: &Self) -> bool {
        core::ptr::eq(self, other)
    }
}

impl<T, const C: u8, I: Indices> Eq for MmapStorage<T, C, I> {}

// code above should go to "diy", code below should stay here.

type Inner<T> = MmapStorage<T, { Calc::PowerOfTwo as u8 }, CachePaddedIndices>;

#[derive(Debug)]
pub struct RingBuffer<T>(PhantomData<Inner<T>>);

impl<T> RingBuffer<T> {
    #[allow(clippy::new_ret_no_self)]
    pub fn new(capacity: usize) -> (Producer<T>, Consumer<T>) {
        let (p, c) = Inner::<T>::new(capacity);
        (Producer(p), Consumer(c))
    }
}

#[derive(Debug, PartialEq, Eq)]
pub struct Producer<T>(crate::diy::Producer<Ptr<Inner<T>>>);

impl<T> Producer<T> {
    pub fn push(&mut self, value: T) -> Result<(), PushError<T>> {
        self.0.push(value)
    }
    pub fn capacity(&self) -> usize {
        self.0.capacity()
    }
}

#[derive(Debug, PartialEq, Eq)]
pub struct Consumer<T>(crate::diy::Consumer<Ptr<Inner<T>>>);

impl<T> Consumer<T> {
    pub fn pop(&mut self) -> Result<T, PopError> {
        self.0.pop()
    }
    pub fn read_chunk(&mut self, n: usize) -> Result<ReadChunk<'_, T>, ChunkError> {
        // SAFETY: MmapStorage guarantees enough valid data for ReadChunkOneSlice.
        unsafe { self.0.read_chunk(n).map(ReadChunk) }
    }

    pub fn slots(&self) -> usize {
        self.0.slots()
    }

    pub fn capacity(&self) -> usize {
        self.0.capacity()
    }
}

#[derive(Debug, PartialEq, Eq)]
pub struct ReadChunk<'a, T>(crate::diy::chunks::ReadChunkOneSlice<'a, Ptr<Inner<T>>>);

impl<T> ReadChunk<'_, T> {
    #[must_use]
    pub fn as_slice(&self) -> &[T] {
        self.0.as_slice()
    }

    pub fn as_mut_slice(&mut self) -> &mut [T] {
        self.0.as_mut_slice()
    }

    pub fn commit(self, n: usize) {
        self.0.commit(n)
    }

    pub fn commit_all(self) {
        self.0.commit_all()
    }

    pub fn len(&self) -> usize {
        self.0.len()
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}
