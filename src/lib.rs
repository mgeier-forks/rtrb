//! A collection of realtime-safe single-producer single-consumer (SPSC) ring buffers.
//!
//! *If you are looking for the ring buffer formerly plainly known as `rtrb::RingBuffer`,
//! this is now available as [`rtrb::arc::RingBuffer`](arc::RingBuffer).
//! Use this if you can't decide!*
//!
//! The following table gives an overview about the available types of ring buffer.
//! Further details are explained in the sections below.
//!
//! | module | storage | reference counted | cacheline padded | contiguous chunks |
//! |-|-|-|-|-|
//! | [`arc`]<br>[`arc2`] | heap | ✔️ | ✔️ ||
//! | [`mod@slice`] | heap (or wherever) || ✔️ ||
//! | [`mod@array`] | array || ✔️ ||
//! | [`array_unpadded`]<br>[`array_unpadded2`] | array ||||
//! | [`bip_arc`]<br>[`bip_arc2`] | heap | ✔️ | ✔️ | ✔️ |
//! | [`bip_slice`] | heap (or wherever) || ✔️ | ✔️ |
//! | [`bip_array`] | array || ✔️ | ✔️ |
//! | [`bip_array_unpadded`]<br>[`bip_array_unpadded2`] | array ||| ✔️ |
//! | [`vrb_arc2`] | virtual memory | ✔️ | ✔️ | ✔️ |
//!
//! Modules ending with `2` use capacities that are powers of two.
//!
//! Modules containing `arc` require the `alloc` feature (which is enabled by default),
//! [`rtrb::vrb_arc2`](vrb_arc2) requires the `vrb` feature.
//!
//! # A Quick Example
//!
//! ```no_run
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
//! You can find this example (and more!) at
//! <https://github.com/mgeier/rtrb/blob/main/examples/>.
//!
//! # General Properties
//!
//! With all ring buffers in this crate,
//! only a single thread can write into the ring buffer and a single thread
//! (typically a different one) can read from it.
//! This setup is called *single producer single consumer* (SPSC).
//! This is of course less flexible than allowing multiple producers and/or consumers,
//! but on the flip side the implementation is much simpler and also faster.
//!
//! ... some modules are faster than others -> power of 2
//!
//! Reading from and writing into the ring buffer is always *lock-free* and *wait-free*.
//! All reading and writing functions return immediately.
//! Attempts to write to a full buffer return an error;
//! values inside the buffer are *not* overwritten.
//! Attempts to read from an empty buffer return an error as well.
//! This means that if the ring buffer is empty, there is no way for the reading thread to wait
//! for new data, other than trying repeatedly until reading succeeds.
//! Similarly, if the ring buffer is full, there is no way for the writing thread
//! to wait for newly available space to write to, other than trying repeatedly.
//!
//! All ring buffers in this crate are *bounded*,
//! which means that their capacity is set once and cannot grow.
//!
//! ... wrap-around
//!
//! ... single element vs chunks -> contiguous chunks
//!
//!
//! # Storage
//!
//! The modules containing the word `array`
//! are using the built-in
//! [`prim@array`] type for storing ring buffer elements.
//! This means that the capacity must be known at compile time.
//! No dynamic memory is ever allocated.
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
//! let mut producer: Producer<'static, i32> = RB.producer().unwrap();
//!
//! // You can create producer and consumer in different threads, or you can
//! // move them to other threads afterwards.
//! let mut consumer: Consumer<'static, i32> = RB.consumer().unwrap();
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
//! ... no other memory allocations ...
//!
//!
//! # Reference Counted
//!
//! All modules containing `arc` have a reference-counted producer and consumer.
//! Calling this "reference-counted" is probably a bit of a stretch,
//! since the counters for producer and consumer can both at most reach `1`,
//! which they immediately do upon ring buffer creation,
//! which creates and returns a producer/consumer pair
//! (see e.g. [`rtrb::arc::RingBuffer::new()`](arc::RingBuffer::new)).
//! The `RingBuffer` object itself is not directly accessible.
//! Once both the producer and the consumer are being dropped
//! (i.e. both their reference counts reach `0`), the whole `RingBuffer` is dropped.
//!
//! All other modules (i.e. the ones *not* containing `arc`)
//! do provide access to the `RingBuffer` in some form,
//! which then provides methods to create a producer and a consumer
//! (see e.g. [`rtrb::array::RingBuffer::producer()`](array::RingBuffer::producer)).
//! In those modules, dropping producer and consumer does *not* automatically drop the `RingBuffer`.
//! All non-`arc` producers and consumers have a lifetime argument that
//! (unless `'static`) infects all data structures which contain them.
//!
//!
//! # Cacheline Padded
//!
//! Unless their module name contains `unpadded`, all ring buffers use
//! [`CachePadded`](https://docs.rs/crossbeam-utils/latest/crossbeam_utils/struct.CachePadded.html)
//! (vendored from [crossbeam-utils](https://crates.io/crates/crossbeam-utils))
//! around the internal atomic "read" and "write" indices.
//! This avoids [false sharing](https://en.wikipedia.org/wiki/False_sharing)
//! between the producer and the consumer thread.
//!
//! On (embedded) systems without coherent caches shared between cores
//! (or with only one core) this is not relevant and the memory overhead can be avoided
//! by using the `unpadded` variants, e.g. [`rtrb::array_unpadded`](array_unpadded).
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
//! If you are willing to use some `unsafe` code,
//! you can use the `*array*` and `*slice*` modules
//! to communicate between processes via shared memory.
//!
//! Have a look at the [`shared-memory-array` example application](https://github.com/mgeier/rtrb/blob/main/examples/shared-memory-array.rs),
//! which you can run with:
//!
//! ```text
//! cargo run --example shared-memory-array
//! ```
//!
//! ... and the [`shared-memory-slice` example application](https://github.com/mgeier/rtrb/blob/main/examples/shared-memory-slice.rs),
//! which you can run with:
//!
//! ```text
//! cargo run --example shared-memory-slice
//! ```
//!
//!
//! # Contiguous Chunks
//!
//! TODO
//!
//!
//! # Usage in Bare-Metal/Embedded Systems
//!
//! ... `no_std`
//!
//! ... multiple aspects: `alloc` availability, support for atomic operations,
//! cacheline padding, power-of-two indices, TODO: DMA
//!
//! ... padding only with producer and consumer on different cores with coherent caches ...
//!
//! ... STM32MP15x: two cache-coherent Cortex-A7 cores (`armv7a-none-eabihf`),
//! one not cache-coherent Cortex-M4 coprocessor (`thumbv7em-none-eabihf`)
//!
//! ... for example STM32H7 is a dual-core MCU (Cortex-M7 + Cortex-M4, both `thumbv7em`),
//! but those cores are not cache-coherent,
//! so the ring buffer should live in a non-cacheable MPU region
//! and doesn't need cacheline padding.
//!
//! ... the padded variants still work, but they will use more memory than necessary.
//!
//! Modules containing `arc` need `target_has_atomic`,
//! all others work on hardware without *read-modify-write* (RMW) support,
//! e.g. `thumbv6m` (Cortex-M0/M0+), `riscv32imc` (RP2040, STM32F0/L0, nRF51).
//! No critical sections are used anywhere, so everything is still wait-free.
//!
//! ... even though the documentation uses the term "thread" ...
//! a single CPU core in a microcontrollers ... no OS threads ...
//! communicate between "main code" and "interrupt handler" ...
//!
//! main loop ↔ ISR
#![doc(
    html_favicon_url = "https://raw.githubusercontent.com/mgeier/rtrb/refs/heads/main/favicon.svg"
)]
#![doc(
    html_logo_url = "https://raw.githubusercontent.com/mgeier/rtrb/refs/heads/main/rtrb-logo.svg"
)]
#![cfg_attr(not(feature = "std"), no_std)]
#![deny(missing_docs, missing_debug_implementations)]
// Add "Available on crate feature ... only." on docs.rs (where applicable).
#![cfg_attr(docsrs, feature(doc_auto_cfg))]

#[cfg(feature = "alloc")]
extern crate alloc;

use core::{fmt, mem::MaybeUninit};

#[allow(dead_code, clippy::undocumented_unsafe_blocks)]
#[cfg(not(doctest))]
mod cache_padded;
/// Dummy module to avoid failing cache_padded doctests.
#[cfg(doctest)]
mod cache_padded {
    pub struct CachePadded;
}

mod atomic {
    pub use core::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
}

#[macro_use]
mod dst_instantiation;

#[cfg(feature = "alloc")]
pub mod arc;
#[cfg(feature = "alloc")]
pub mod arc2;
pub mod array;
pub mod array_unpadded;
pub mod array_unpadded2;
#[cfg(feature = "alloc")]
pub mod bip_arc;
#[cfg(feature = "alloc")]
pub mod bip_arc2;
pub mod bip_array;
pub mod bip_array_unpadded;
pub mod bip_array_unpadded2;
pub mod bip_slice;
pub mod slice;
#[cfg(feature = "vrb")]
pub mod vrb_arc2;

// For backwards compatibility. May be deprecated and removed in the future.
#[cfg(feature = "alloc")]
#[doc(hidden)]
pub use arc::{Consumer, Producer, RingBuffer};

// For backwards compatibility. May be deprecated and removed in the future.
#[cfg(feature = "alloc")]
#[doc(hidden)]
pub mod chunks {
    pub use super::arc::chunks::{ReadChunk, ReadChunkIntoIter, WriteChunk, WriteChunkUninit};
    pub use super::arc::ChunkError;
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

impl fmt::Display for ChunkError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
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
