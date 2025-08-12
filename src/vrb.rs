//! ...
//!
//! Same as [`rtrb::vrb_arc`](crate::vrb_arc), but ...

// `pow2 = no` should work as well, but the capacity will always be a power of two
// (a multiple of (page size / size of `T`)), so `pow2 = yes` probably makes more sense.

ring_buffer! {
    storage = vrb,
    N = no,
    arc = no,
    bip = no,
    contiguous = yes,
    padded = yes,
    pow2 = yes,
    module = "rtrb::vrb",
    rb_doc = docstring!(
        /// Ring buffer ... virtual memory ...
        ///
        /// TODO: ...
        ///
        /// *See also the [module-level documentation](crate::vrb).*
    )
}
