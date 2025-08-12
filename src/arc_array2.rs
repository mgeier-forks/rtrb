//! An array on the heap.
//!
//! This may not be very useful, but who knows?

ring_buffer! {
    storage = array,
    N = yes,
    arc = yes,
    bip = no,
    contiguous = no,
    padded = yes,
    pow2 = yes,
    module = "rtrb::arc_array",
    rb_doc = docstring!(
        /// A ring buffer with elements stored in an [array] (with a power-of-2 size) on the heap.
        ///
        /// *See also the [module-level documentation](crate::arc_array).*
    )
}
