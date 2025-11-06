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

// NB: The capacity will always be rounded up to a power of two
// (a multiple of (page size / size of `T`)), so `pow2 = no` doesn't make sense.

ring_buffer! {
    storage = vrb,
    N = no,
    arc = yes,
    bip = no,
    contiguous = yes,
    padded = yes,
    pow2 = yes,
    module = "rtrb::vrb_arc2",
    rb_doc = docstring!(
        /// Ring buffer ... virtual memory ...
        ///
        /// TODO: ...
        ///
        /// ... `capacity` will be rounded up to page size ... (TODO: add this in constructor docs?)
        ///
        /// *See also the [module-level documentation](crate::vrb_arc2).*
    )
}
