//! ...
//!
//! no dynamic allocation, but cache-padded indices

ring_buffer! {
    storage = array,
    N = yes,
    arc = no,
    bip = no,
    contiguous = no,
    padded = yes,
    pow2 = no,
    module = "rtrb::array",
    rb_doc = docstring!(
        /// Ring buffer using an array as storage.
        ///
        /// TODO: ...
        ///
        /// *See also the [module-level documentation](crate::array).*
    )
}
