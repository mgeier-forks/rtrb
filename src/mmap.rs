use core::{convert::TryInto, marker::PhantomData, sync::atomic::AtomicU8};

use crate::{
    diy::{Addressing, Indices, Storage},
    CachePaddedIndices, Ptr,
};

// TODO: move MmapStorage to "diy" module?

#[derive(Debug)]
pub struct MmapStorage<T, const A: u8, I: Indices> {
    indices: I,

    flags: AtomicU8,

    /// Pointer to the first mapped region
    data_ptr: *mut T,

    capacity: usize,

    /// Indicates that dropping a `MmapStorage` may drop elements of type `T`.
    _marker: PhantomData<T>,
}

/// `T` is not `Sync` because we never share it across threads.
// SAFETY: There is not mutable state (except for interior mutability).
unsafe impl<T: Send, const A: u8, I: Indices + Sync> Sync for MmapStorage<T, A, I> {}

// NB: MmapStorage doesn't need to be `Send` because it is never moved.

// Any `Addressing` should work, but the capacity will always be a power of two
// (a multiple of (page size / size of `T`)),
// so `PowerOfTwoAddressing` probably makes most sense.
impl<T, const A: u8, I: Indices> MmapStorage<T, A, I> {
    #[allow(clippy::new_ret_no_self)]
    pub fn new(
        capacity: usize,
    ) -> (
        crate::diy::Producer<Ptr<MmapStorage<T, A, I>>>,
        crate::diy::Consumer<Ptr<MmapStorage<T, A, I>>>,
    ) {
        // TODO: what if capacity is 0?
        let pagesize = unsafe { libc::sysconf(libc::_SC_PAGESIZE) };
        assert_ne!(pagesize, -1);
        let size_of_t = core::mem::size_of::<T>();
        let elements_per_page = TryInto::<usize>::try_into(pagesize).unwrap() / size_of_t;
        let rem = TryInto::<usize>::try_into(pagesize).unwrap() % size_of_t;
        assert_eq!(rem, 0);
        let pages = (capacity / elements_per_page) + (capacity % elements_per_page > 0) as usize;
        let capacity = pages * elements_per_page;
        assert_eq!(capacity, Addressing::from_u8(A).update_capacity(capacity));
        assert_eq!(capacity, capacity.next_power_of_two());
        let len = capacity * core::mem::size_of::<T>();
        let data_ptr: *mut T = unsafe {
            use libc::*;
            let fd = memfd_create(
                core::ffi::CStr::from_bytes_with_nul(b"rtrb-buffer\0")
                    .unwrap()
                    .as_ptr(),
                0,
            );
            ftruncate(fd, TryInto::<off_t>::try_into(len).unwrap());
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

impl<T, const A: u8, I: Indices> Drop for MmapStorage<T, A, I> {
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

// SAFETY: all methods must be implemented correctly, or the whole thing is unsound
unsafe impl<T, const A: u8, I: Indices> Storage for MmapStorage<T, A, I> {
    type Item = T;
    type Indices = I;
    const ADDR: Addressing = Addressing::from_u8(A);

    #[inline]
    fn indices(&self) -> &Self::Indices {
        &self.indices
    }

    fn flags(&self) -> &AtomicU8 {
        &self.flags
    }

    #[inline]
    fn data_ptr(&self) -> *mut Self::Item {
        self.data_ptr
    }

    #[inline]
    fn capacity(&self) -> usize {
        self.capacity
    }
}

pub type MmapRingBuffer<T> = MmapStorage<T, { Addressing::PowerOfTwo as u8 }, CachePaddedIndices>;
