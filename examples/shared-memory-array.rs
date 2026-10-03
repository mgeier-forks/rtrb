#![allow(clippy::incompatible_msrv)] // for NonNull methods (stable since 1.80.0)

use std::{io::Write as _, marker::PhantomData, ptr::NonNull, time::Duration};

use rtrb::array::{Consumer, Producer, RingBuffer};
use shared_memory::{Shmem, ShmemConf, ShmemError};

type Buffer = RingBuffer<i32, 16>;

fn cast_ptr(shmem: &Shmem) -> NonNull<Buffer> {
    assert!(shmem.len() >= std::mem::size_of::<Buffer>());
    let ptr = NonNull::new(shmem.as_ptr()).unwrap().cast();
    // NB: ptr.align_offset() could be used to guarantee alignment,
    //     but this would need some more careful considerations.
    assert!(ptr.is_aligned());
    ptr
}

struct Owner<'a> {
    ptr: NonNull<Buffer>,
    _phantom: PhantomData<&'a Shmem>,
}

impl Owner<'_> {
    /// Owns a ring buffer that lives in shared memory.
    ///
    /// Takes its lifetime from `shmem`.
    ///
    /// # Safety
    ///
    /// `shmem` must point to unused, writable memory.
    unsafe fn new(shmem: &Shmem) -> Self {
        let ptr = cast_ptr(shmem);
        // SAFETY: see docstring.
        unsafe {
            ptr.write(Buffer::new());
        }
        Self {
            ptr,
            _phantom: PhantomData,
        }
    }

    fn buffer(&self) -> &Buffer {
        // SAFETY: Caller of new() must make sure the memory is valid.
        unsafe { self.ptr.as_ref() }
    }

    fn producer(&self) -> Option<Producer<'_, i32>> {
        self.buffer().producer()
    }
}

// NB: This is not really necessary in this example,
// it is only relevant if the stored type needs to be dropped.
impl Drop for Owner<'_> {
    fn drop(&mut self) {
        if let Some(mut c) = self.buffer().consumer() {
            // All remaining elements are dropped here.
            c.read_chunk(c.slots()).unwrap().commit_all();
        } else {
            println!("Owner is dropped while consumer is still connected.");
        }
    }
}

/// Get a Consumer from initialized shared memory.
///
/// The consumer takes its lifetime from the shared memory.
///
/// # Safety
///
/// `shmem` must point to correctly initialized memory.
unsafe fn get_consumer(shmem: &Shmem) -> Option<Consumer<'_, i32>> {
    let ptr = cast_ptr(shmem);
    // SAFETY: see docstring.
    unsafe { ptr.as_ref().consumer() }
}

fn sleep() {
    std::thread::sleep(Duration::from_millis(100));
}

enum Error {
    Shmem(ShmemError),
    NoConnection,
    AlreadyConsuming,
}

impl std::fmt::Debug for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Error::Shmem(e) => e.fmt(f),
            Error::NoConnection => write!(f, "connection could not be established"),
            Error::AlreadyConsuming => write!(
                f,
                "another process is already connected \
                (or the file `delete-me` has to be removed?)"
            ),
        }
    }
}

impl From<ShmemError> for Error {
    fn from(value: ShmemError) -> Self {
        Error::Shmem(value)
    }
}

macro_rules! print_flush {
    ($($t:tt)*) => {{
        let mut h = ::std::io::stdout();
        write!(h, $($t)*).unwrap();
        h.flush().unwrap();
    }}
}

fn main() -> Result<(), Error> {
    let size = std::mem::size_of::<Buffer>();
    let name = "delete-me";
    match ShmemConf::new().size(size).flink(name).create() {
        Ok(shmem) => {
            // SAFETY: shared memory has the right size, alignment and lifetime.
            let owner = unsafe { Owner::new(&shmem) };
            let mut p = owner.producer().unwrap();
            println!("please start the same program again (in another terminal)");
            print!("waiting for connection ...");
            let mut counter = 0;
            while !p.has_consumer() {
                if counter == 100 {
                    println!();
                    return Err(Error::NoConnection);
                }
                print_flush!(".");
                sleep();
                counter += 1;
            }
            println!(" connected.");
            print_flush!("sending data ...");
            sleep();
            for i in 0..42 {
                while p.push(i).is_err() {
                    sleep();
                }
                print_flush!(" {i}");
                sleep();
            }
            println!();

            drop(p); // This signals to the other process that no more data is coming.

            // NB: We are now waiting for the other process to finish
            // before we let the ring buffer go out of scope.
            //
            // However, in this very case this is not strictly necessary
            // because the payload type `i32` does not need dropping.
            // So even if the `RingBuffer` object is dropped, the numbers in it will remain
            // in the shared memory, which will be available until the last process exits.
            //
            // Remove the remaining lines in this code block to try it out!
            // You should see a message "Owner is dropped ...".

            print!("waiting for disconnection ...");
            while owner.buffer().has_consumer() {
                sleep();
                print_flush!(".");
            }
            println!(" done.");
        }
        Err(ShmemError::LinkExists) => {
            println!("connecting to other process ...");
            println!("(remove the file `delete-me` if this is the only process)");
            let shmem = ShmemConf::new().flink(name).open()?;
            // SAFETY: memory has been initialized with Owner::new().
            // NB: There is a very short time span when the file already exists,
            // but the RingBuffer is not fully initialized yet.
            let mut c = unsafe { get_consumer(&shmem) }.ok_or(Error::AlreadyConsuming)?;
            if !c.has_producer() {
                // The other process has given up in the meantime.
                return Err(Error::NoConnection);
            }
            print_flush!("receiving data ...");
            loop {
                if let Ok(value) = c.pop() {
                    print_flush!(" {value}");
                } else if !c.has_producer() && c.is_empty() {
                    // NB: is_empty() checks for the very unlikely case where the sender
                    // pushes a value and drops the producer right after we called pop()
                    // (and found an empty queue) but before our call to has_producer().
                    break;
                }
                // We receive slower than we are sending:
                sleep();
                sleep();
                sleep();
            }
            println!(" done.");
            println!("disconnecting.");
        }
        Err(e) => return Err(e.into()),
    }
    Ok(())
}
