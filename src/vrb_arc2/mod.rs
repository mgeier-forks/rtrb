// This file has been auto-generated ...


//! TODO: non-bip ring buffer.

//! Ring buffer using a virtual memory trick.
//!
//! Phil Howard is maybe the inventor (2001?):
//! <http://web.archive.org/web/20190208212054/http://freshmeat.sourceforge.net/projects/vrb/>
//!
//! <http://web.archive.org/web/20140705114711/http://vrb.sourceforge.net/>
//!
//! code available here (as part of LIBH): <https://web.archive.org/web/20140625200016/http://libh.slashusr.org/>
//!
//! Potential Windows solution:
//! <https://fgiesen.wordpress.com/2012/07/21/the-magic-ring-buffer/>
//!
//! ... `capacity` will be rounded up to page size ... (TODO: add this in constructor docs?)

/// A bounded single-producer single-consumer (SPSC) queue.
///
/// Elements can be written with a [`Producer`] and read with a [`Consumer`],
/// both of which can be obtained with [`RingBuffer::new()`].
///
/// *See also the [module-level documentation](rtrb::vrb_arc2).*
pub struct RingBuffer<T> {
}

