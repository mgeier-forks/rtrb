//! Ring buffer, array, power of 2.

ring_buffer! {
    storage = array,
    N = yes,
    arc = no,
    bip = no,
    contiguous = no,
    padded = yes,
    pow2 = yes,
    module = "rtrb::array2",
    rb_doc = docstring!(
        /// Ring buffer using an array with a power-of-two size as storage.
        ///
        /// *See also the [module-level documentation](crate::array2).*
    )
}
