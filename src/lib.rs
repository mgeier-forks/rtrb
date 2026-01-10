//! A collection of realtime-safe single-producer single-consumer (SPSC) ring buffers.
//!
//! *If you are looking for the ring buffer formerly plainly known as `rtrb::RingBuffer`,
//! this is now available as [`rtrb::arc::RingBuffer`](arc::RingBuffer).*
//!
//! The following table gives an overview about the available types of ring buffer.
//! Further details are explained in the sections below.
//!
//! | module | storage | reference counted | cache padded | contiguous chunks |
//! |-|-|-|-|-|
//! | [`arc`]/[`arc2`] | heap | ✔️ | ✔️ ||
//! | [`mod@array`] | array || ✔️ ||
//! | [`embedded`] | array ||||
//! | [`bip_arc`]/[`bip_arc2`] | heap | ✔️ | ✔️ | ✔️ |
//! | [`bip_array`] | array || ✔️ | ✔️ |
//! | [`vrb_arc2`] | mmap | ✔️ | ✔️ | ✔️ |
//!
//!
//! # A Quick Example
//!
//! ```no_run
#![allow(clippy::needless_doctest_main)]
#![doc = include_str!("../examples/quick.rs")]
//! ```
//!
//! You can run this program locally with just a few steps
//! (assuming [Rust](https://rustup.rs/) and [Git](https://git-scm.com/) is installed):
//!
//! ```text
//! git clone https://github.com/mgeier/rtrb.git
//! cd rtrb
//! cargo run --example quick
//! ```
//!
//! <details>
//! <summary>Possible program output</summary>
//!
//! ```text
//! waiting to pop ...
//! pushed 10
//! popped 10
//! pushed 11
//! pushed 12
//! popped 11
//! pushed 13
//! pushed 14
//! popped 12
//! pushed 15
//! pushed 16
//! popped 13
//! pushed 17
//! 18 was skipped
//! popped 14
//! pushed 19
//! 20 was skipped
//! popped 15
//! pushed 21
//! 22 was skipped
//! popped 16
//! pushed 23
//! 24 was skipped
//! popped 17
//! popped 19
//! popped 21
//! popped 23
//! waiting to pop ...
//! waiting to pop ...
//! waiting to pop ...
//! giving up
//! ```
//!
//! </details>
//!
//! # General Properties
//!
//! ... SPSC ... bounded ... wrap-around ... single element vs chunks? ...
//!
//! Reading from and writing into the ring buffer is *lock-free* and *wait-free*.
//! All reading and writing functions return immediately.
//! Attempts to write to a full buffer return an error;
//! values inside the buffer are *not* overwritten.
//! Attempts to read from an empty buffer return an error as well.
//!
//! Only a single thread can write into the ring buffer and a single thread
//! (typically a different one) can read from the ring buffer.
//! If the queue is empty, there is no way for the reading thread to wait
//! for new data, other than trying repeatedly until reading succeeds.
//! Similarly, if the queue is full, there is no way for the writing thread
//! to wait for newly available space to write to, other than trying repeatedly.
//!
//!
//! # Storage
//!
//! The modules containing the word `array` (TODO: as well as `embedded`?)
//! are using the built-in
//! [`prim@array`] type for storing ring buffer elements.
//! This means that the capacity must be known at compile time.
//! No dynamic memory is ever allocated
//! (except in `Display` impls of some error messages,
//! but only when the `alloc` feature is enabled).
//!
//! TODO: example with and without N
//!
//! ... have the advantage that their ring buffers can be used as `static` variables.
//!
//! Here's an example using the [`rtrb::array`](mod@array) module:
//!
//! ```
//! use rtrb::array::{Consumer, Producer, RingBuffer};
//!
//! static RB: RingBuffer<i32, 64> = RingBuffer::new();
//!
//! let mut p: Producer<'static, i32> = RB.producer().unwrap();
//!
//! // You can create producer and consumer in different threads, or you can
//! // create them in the same thread and move them to separate threads afterwards.
//! let mut c: Consumer<'static, i32> = RB.consumer().unwrap();
//! ```
//!
//! A disadvantage ... size restrictions (stack size) ...
//!
//! All other modules use dynamic memory (allocated on the
//! [heap](https://doc.rust-lang.org/book/ch04-01-what-is-ownership.html#the-stack-and-the-heap)).
//!
//! ... no stack size restrictions ...
//!
//! ... since the compiler doesn't know the size, potentially fewer optimizations ...
//!
//! A fixed-capacity buffer is allocated on construction.
//!
//! Memory is only allocated once, when creating the `RingBuffer`.
//!
//! After that, no more memory is allocated (unless the type `T` does that internally).
//!
//! ... no other memory allocations ... (TODO: except for `Display` impls of some error messages?)
//!
//!
//! # Reference Counted
//!
//! TODO
//!
//!
//! # Cache Padded
//!
//! TODO
//!
//!
//! # Calculation of Indices
//!
//! ... head/tail ... in range `0 .. 2 * capacity`, `pow2`: no limit except wrap-around ...
//!
//! Indices are wrapped at twice the buffer size.
//!
//! Indices are wrapped at [`usize::MAX`].
//!
//! Other approaches are used in the wild ... wasting one slot ...
//!
//!
//! # Usage with Shared Memory
//!
//! ... [`shared-memory` example application](https://github.com/mgeier/rtrb/blob/main/examples/shared-memory.rs) ...
//!
//! ```text
//! cargo run --example shared-memory
//! ```
//!
//! TODO: shared memory with DST?
//!
//! # Usage in Embedded Systems
//!
//! ... no cache padding ...
//!
//! ... even though the documentation uses the term "thread" ...
//! a single CPU core in a microcontrollers ... no OS threads ...
//! communicate between "main code" and "interrupt handler" ...
//!
//! TODO: DMA? use "bip" variants.

#![cfg_attr(not(feature = "std"), no_std)]
#![deny(missing_docs, missing_debug_implementations)]
#![deny(unsafe_op_in_unsafe_fn)]
#![warn(clippy::undocumented_unsafe_blocks, clippy::unnecessary_safety_comment)]
// Add "Available on crate feature ... only." on docs.rs (where applicable).
#![cfg_attr(docsrs, feature(doc_auto_cfg))]

#[cfg(feature = "alloc")]
extern crate alloc;

use core::{fmt, mem::MaybeUninit};

#[allow(dead_code, clippy::undocumented_unsafe_blocks)]
mod cache_padded;
/// TODO: public re-export, see crossbeam-utils
#[doc(inline)]
pub use cache_padded::CachePadded;

// TODO: feature "portable-atomic"?
mod atomic {
    pub use core::sync::atomic::{AtomicU8, AtomicUsize, Ordering};
}

#[cfg(feature = "alloc")]
// This is only used if storage == "dst"
#[warn(unused_imports)]
#[macro_use]
mod dst_instantiation;

#[cfg(feature = "alloc")]
pub mod arc;
#[cfg(feature = "alloc")]
pub mod arc2;
pub mod array;
#[cfg(feature = "alloc")]
pub mod dst_arc;
#[cfg(feature = "alloc")]
pub mod bip_arc;
#[cfg(feature = "alloc")]
pub mod bip_arc2;
pub mod bip_array;
pub mod embedded;
#[cfg(feature = "vrb")]
pub mod vrb_arc2;

// TODO: boxed, from_ptr, new_at_ptr (+ same for bip)

// For backwards compatibility. May be deprecated and removed in the future.
#[cfg(feature = "alloc")]
#[doc(hidden)]
pub use arc::{Consumer, Producer, RingBuffer};

// For backwards compatibility. May be deprecated and removed in the future.
#[cfg(feature = "alloc")]
#[doc(hidden)]
pub mod chunks {
    pub use crate::arc::chunks::{ReadChunk, ReadChunkIntoIter, WriteChunk, WriteChunkUninit};
    pub use crate::ChunkError;
}

#[doc(hidden)]
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum PopError {
    /// The queue was empty.
    Empty,
}

#[cfg(feature = "std")]
impl std::error::Error for PopError {}

impl fmt::Display for PopError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PopError::Empty => "empty ring buffer".fmt(f),
        }
    }
}

#[doc(hidden)]
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum PeekError {
    /// The queue was empty.
    Empty,
}

#[cfg(feature = "std")]
impl std::error::Error for PeekError {}

impl fmt::Display for PeekError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PeekError::Empty => "empty ring buffer".fmt(f),
        }
    }
}

#[doc(hidden)]
#[derive(Copy, Clone, PartialEq, Eq)]
pub enum PushError<T> {
    /// The queue was full.
    Full(T),
}

#[cfg(feature = "std")]
impl<T> std::error::Error for PushError<T> {}

impl<T> fmt::Debug for PushError<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PushError::Full(_) => f.pad("Full(_)"),
        }
    }
}

impl<T> fmt::Display for PushError<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PushError::Full(_) => "full ring buffer".fmt(f),
        }
    }
}

#[doc(hidden)]
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum ChunkError {
    /// Fewer than the requested number of slots were available.
    ///
    /// Contains the number of slots that were available.
    TooFewSlots(usize),
}

#[cfg(feature = "std")]
impl std::error::Error for ChunkError {}

// TODO: a version without "alloc" feature.
impl fmt::Display for ChunkError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            #[cfg(feature = "alloc")]
            ChunkError::TooFewSlots(n) => {
                alloc::format!("only {n} slots available in ring buffer").fmt(f)
            }
            #[cfg(not(feature = "alloc"))]
            ChunkError::TooFewSlots(_) => "too few slots available in ring buffer".fmt(f),
        }
    }
}

#[doc(hidden)]
pub trait CopyToUninit<T: Copy> {
    /// Copies contents to a possibly uninitialized slice.
    fn copy_to_uninit<'a>(&self, dst: &'a mut [MaybeUninit<T>]) -> &'a mut [T];
}

impl<T: Copy> CopyToUninit<T> for [T] {
    /// Copies contents to a possibly uninitialized slice.
    ///
    /// # Panics
    ///
    /// This function will panic if the two slices have different lengths.
    fn copy_to_uninit<'a>(&self, dst: &'a mut [MaybeUninit<T>]) -> &'a mut [T] {
        assert_eq!(
            self.len(),
            dst.len(),
            "source slice length does not match destination slice length"
        );
        let dst_ptr = dst.as_mut_ptr().cast();
        // SAFETY: The lengths have been checked to be equal and
        // the mutable reference makes sure that there is no overlap.
        unsafe {
            self.as_ptr().copy_to_nonoverlapping(dst_ptr, self.len());
            core::slice::from_raw_parts_mut(dst_ptr, self.len())
        }
    }
}
