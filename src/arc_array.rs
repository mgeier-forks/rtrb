//! An array on the heap.
//!
//! This may not be very useful, but who knows?
//!
//! ... `static` can be used with `array` ...
//!
//! ... even though ultimately allocated on the stack,
//! the maximum buffer size is still limited by the stack size ...

storage_array! {
    arc = yes,
    padded = yes,
    bip = no,
    rb_doc = "
A ring buffer with elements stored in an [array] on the heap.

*See also the [module-level documentation](crate::arc_array).*
"
}

impl_everything_eventually! {
    storage = array,
    N = yes,
    arc = yes,
    bip = no,
    contiguous = no,
    pow2 = no,
    module = "rtrb::arc_array",
}
