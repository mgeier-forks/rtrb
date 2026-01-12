use std::io::Write as _;
use std::time::Duration;

use rtrb::dst_box::{Consumer, Producer, RingBuffer};
use shared_memory::{Shmem, ShmemConf, ShmemError};

type Buffer = RingBuffer<i32>;

fn validate_ptr(shmem: &Shmem, capacity: usize) -> *mut u8 {
    let layout = Buffer::layout(capacity);
    assert!(shmem.len() >= layout.size());
    let ptr = shmem.as_ptr();
    // NB: ptr.align_offset() could be used to guarantee alignment,
    //     but this would need some more careful considerations.
    // NB: ptr.is_aligned_to() is unstable, so we use this work-around:
    assert!(ptr as usize & (layout.align() - 1) == 0);
    ptr
}

struct Owner<'a>(&'a Buffer);

impl Owner<'_> {
    /// Owns a ring buffer that lives in shared memory.
    ///
    /// Takes its lifetime from `shmem`.
    ///
    /// # Safety
    ///
    /// `shmem` must point to unused, writable memory.
    unsafe fn new(shmem: &Shmem, capacity: usize) -> Self {
        let ptr = validate_ptr(shmem, capacity);
        Self(Buffer::new_at(ptr, capacity))
    }

    fn buffer(&self) -> &Buffer {
        self.0
    }

    fn producer(&self) -> Option<Producer<'_, i32>> {
        self.buffer().producer()
    }
}

impl Drop for Owner<'_> {
    fn drop(&mut self) {
        // NB: This is not really necessary here,
        // it is only relevant if T implements Drop.
        let ptr = self.0 as *const _ as *mut Buffer;
        // SAFETY: The reference self.0 will not be accessed after this.
        unsafe { ptr.drop_in_place() };
    }
}

/// Get a Consumer from initialized shared memory.
///
/// The consumer takes its lifetime from the shared memory.
///
/// # Safety
///
/// `shmem` must point to correctly initialized memory.
/// `capacity` must be the same as the one used in Buffer::new_at().
unsafe fn get_consumer(shmem: &Shmem, capacity: usize) -> Option<Consumer<'_, i32>> {
    let ptr = validate_ptr(shmem, capacity);
    let rb = Buffer::from_raw_parts(ptr, capacity);
    rb.consumer()
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
    let capacity = 16;
    let layout = Buffer::layout(capacity);
    let name = "delete-me";
    match ShmemConf::new().size(layout.size()).flink(name).create() {
        Ok(shmem) => {
            let owner = unsafe { Owner::new(&shmem, capacity) };
            let mut p = owner.producer().unwrap();
            println!("please start the same program again (in another terminal)");
            print!("waiting for connection ...");
            let mut counter = 0;
            loop {
                if p.has_consumer() {
                    break;
                }
                print_flush!(".");
                sleep();
                counter += 1;
                if counter < 100 {
                    continue;
                } else {
                    println!();
                    return Err(Error::NoConnection);
                }
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
            // because the payload type `i32` does not implement `Drop`.
            // So even if the `RingBuffer` object is dropped, the numbers in it will remain
            // in the shared memory, which will be available until the last process exits.
            //
            // Remove the remaining lines in this clode block to try it out!

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
            let mut c = unsafe { get_consumer(&shmem, capacity) }.ok_or(Error::AlreadyConsuming)?;
            print_flush!("receiving data ...");
            loop {
                if let Ok(value) = c.pop() {
                    print_flush!(" {value}");
                } else if !c.has_producer() {
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
