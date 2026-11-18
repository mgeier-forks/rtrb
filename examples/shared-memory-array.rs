#![allow(clippy::incompatible_msrv)] // for NonNull methods (stable since 1.80.0)

use std::{io::Write as _, marker::PhantomData, ptr::NonNull, time::Duration};

use rtrb::ex::{ArrayRingBuffer, Consumer, Producer};
use shared_memory::{Shmem, ShmemConf, ShmemError};

type Buffer = ArrayRingBuffer<i32, 16>;

fn align_ptr(shmem: &Shmem) -> NonNull<Buffer> {
    let ptr = shmem.as_ptr();
    let offset = ptr.align_offset(std::mem::align_of::<Buffer>());
    assert!(shmem.len() >= std::mem::size_of::<Buffer>() + offset);
    // SAFETY: We just checked that the memory is large enough.
    let ptr = unsafe { ptr.add(offset) };
    NonNull::new(ptr).unwrap().cast()
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
        let ptr = align_ptr(shmem);
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
            // All remaining items are dropped here.
            c.drop_available_items();
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
    let ptr = align_ptr(shmem);
    // SAFETY: see docstring.
    unsafe { ptr.as_ref().consumer() }
}

/// A file that's removed on drop.
struct ScopedFile<'a>(&'a str);

impl<'a> ScopedFile<'a> {
    /// Atomically creates a file, making sure it doesn't exist yet.
    fn new(name: &'a str, contents: &str) -> Result<Self, Error> {
        // A temporary file is written and then hard-linked to the requested name.
        // This avoids the other process reading an incomplete file.
        let tmp_name = format!("{name}.{}.tmp", std::process::id());
        std::fs::write(&tmp_name, contents)?;
        let result = std::fs::hard_link(&tmp_name, name);
        let _ = std::fs::remove_file(&tmp_name);
        result?;
        Ok(Self(name))
    }
}

impl Drop for ScopedFile<'_> {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(self.0);
    }
}

fn sleep() {
    std::thread::sleep(Duration::from_millis(100));
}

enum Error {
    Io(std::io::Error),
    Shmem(ShmemError),
    NoConnection,
    AlreadyConsuming,
}

impl std::fmt::Debug for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Error::Io(e) => e.fmt(f),
            Error::Shmem(e) => e.fmt(f),
            Error::NoConnection => write!(f, "connection could not be established"),
            Error::AlreadyConsuming => write!(
                f,
                "another process is already connected \
                or the other process has just given up waiting \
                (or the file `delete-me` has to be removed?)"
            ),
        }
    }
}

impl From<std::io::Error> for Error {
    fn from(value: std::io::Error) -> Self {
        Error::Io(value)
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
    match std::fs::read_to_string(name) {
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            let shmem = ShmemConf::new()
                .size(size + std::mem::align_of::<Buffer>())
                .create()?;
            // SAFETY: shared memory has the right size and lifetime.
            let owner = unsafe { Owner::new(&shmem) };
            let mut p = owner.producer().unwrap();
            // Now that the ring buffer is fully initialized, we can expose it to the other process:
            let _file = ScopedFile::new(name, shmem.get_os_id())?;
            println!("please start the same program again (in another terminal)");
            print!("waiting for connection ...");
            let mut counter = 0;
            while !p.has_consumer() {
                if counter == 100 {
                    // NB: The other process might connect just after the check above.
                    // To find out, we try to take the consumer ourselves:
                    if let Some(c) = owner.buffer().consumer() {
                        println!();
                        // We have to drop the producer before the consumer,
                        // so that a process connecting afterwards doesn't see it.
                        drop(p);
                        drop(c);
                        return Err(Error::NoConnection);
                    } else {
                        // A consumer has connected in the last moment, which is very unlikely.
                        print!(" it happened in the last moment, but now we are");
                        break;
                    }
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
            // before `owner` is dropped and in turn drops all remaining items.
            //
            // However, in this very case this is not necessary
            // because dropping items of the payload type `i32` does nothing
            // and they will remain in the shared memory,
            // which will be available until the last process exits.
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
        Ok(os_id) => {
            println!("connecting to other process ...");
            println!("(remove the file `delete-me` if this is the only process)");
            let shmem = ShmemConf::new().os_id(os_id).open()?;
            // SAFETY: memory has been initialized with Owner::new() before the file became visible.
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
