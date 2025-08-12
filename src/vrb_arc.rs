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

// `pow2 = no` should work as well, but the capacity will always be a power of two
// (a multiple of (page size / size of `T`)), so `pow2 = yes` probably makes more sense.

ring_buffer! {
    storage = vrb,
    N = no,
    arc = yes,
    bip = no,
    contiguous = yes,
    padded = yes,
    pow2 = yes,
    module = "rtrb::vrb_arc",
    rb_doc = docstring!(
        /// Ring buffer ... virtual memory ...
        ///
        /// TODO: ...
        ///
        /// ... `capacity` is rounded up to page size ... (TODO: add this in constructor docs?)
        ///
        /// *See also the [module-level documentation](crate::vrb_arc).*
    )
}
