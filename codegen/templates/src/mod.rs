// This file has been auto-generated ...

{% include "definitions.jinja" %}

{% if bip %}
{% if module == "rtrb::bip_array" %}
//! A bi-partite ring buffer whose elements are stored in an [array].
//!
//! See [`rtrb::bip_arc`](crate::bip_arc) for a bi-partite ring buffer with dynamic storage.
{% endif %}
{# TODO: correct separation of array vs. pow2 #}
{% if pow2 %}
//! A bi-partite ring buffer whose capacity is a power of two.
{% else %}
//! A bi-partite ring buffer.
//!
//! Simon Cooke (2003)
//! <https://www.codeproject.com/Articles/3479/The-Bip-Buffer-The-Circular-Buffer-with-a-Twist>
//! (not thread-safe)
//!
//! two revolving regions
//!
//! "two-phase allocation system" (reserve + commit)
//!
//! history:
//! "The FIFO logic can tell if the FIFO is empty because the head and tail values are the same, and it's full if the head is one greater than the tail."
//!
//! "Once more free space is available to the left of region A than to the right of it, a second region (comically named "region B") is created in that space."
//!
//! Reserve -> Commit; GetContiguousBlock -> DecommitBlock.
//!
//! 2019:
//! <https://ferrous-systems.com/blog/lock-free-ring-buffer/>
//! <https://blog.systems.ethz.ch/blog/2019/the-design-and-implementation-of-a-lock-free-ring-buffer-with-contiguous-reservations.html>
//!
//! Other Rust implementations:
//! <https://crates.io/crates/bbqueue>
//! <https://crates.io/crates/bipbuffer> (not thread-safe)
//! <https://crates.io/crates/spsc-bip-buffer>
//!
//! Implementations in other languages:
//! <https://github.com/willemt/bipbuffer> (C)
{% endif %}
{% else %}
//! TODO: non-bip ring buffer.
{% endif %}

{% if module == "rtrb::embedded" %}
//! TODO: embedded
//!
//! no cache padding, no dynamic allocation
//! power-of-two optimizations might be done automatically by the compiler? TODO: verify
{% elif module == "rtrb::vrb_arc2" %}
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
{% endif %}

/// A bounded single-producer single-consumer (SPSC) queue.
///
/// Elements can be written with a [`Producer`] and read with a [`Consumer`],
{% if arc %}
/// both of which can be obtained with [`RingBuffer::new()`].
{% else %}
/// which can be obtained with ... TODO
{% endif %}
///
/// *See also the [module-level documentation]({{ module }}).*
{% if N %}
pub struct RingBuffer<T, const N: usize> {
}
{% else %}
pub struct RingBuffer<T> {
}
{% endif %}

{% if not arc %}
impl<{{ params }}> RingBuffer<{{ args }}> {
    pub fn producer(&self) -> Option<Producer<{{ blank_lifetime }}>> {
    }
}
{% endif %}
