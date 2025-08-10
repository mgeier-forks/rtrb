//! ...
//!
//! Same as [`rtrb::vrb_arc`](crate::vrb_arc), but ...

storage_vrb! {
    arc = yes,
    padded = yes,
    rb_doc = "
Ring buffer ... virtual memory ...

TODO: ...

*See also the [module-level documentation](crate::vrb).*
"
}

// `pow2 = no` should work as well, but the capacity will always be a power of two
// (a multiple of (page size / size of `T`)), so `pow2 = yes` probably makes more sense.

impl_everything_eventually! {
    storage = vrb,
    N = no,
    arc = no,
    bip = no,
    contiguous = yes,
    pow2 = yes,
    module = "rtrb::vrb",
}
