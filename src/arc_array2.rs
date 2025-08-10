//! An array on the heap.
//!
//! This may not be very useful, but who knows?

storage_array! {
    arc = yes,
    padded = yes,
    bip = no,
    rb_doc = "
A ring buffer with elements stored in an [array] (with a power-of-2 size) on the heap.

*See also the [module-level documentation](crate::arc_array).*
"
}

impl_everything_eventually! {
    storage = array,
    N = yes,
    arc = yes,
    bip = no,
    contiguous = no,
    pow2 = yes,
    module = "rtrb::arc_array",
}
