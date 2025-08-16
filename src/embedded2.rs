//! ...
//!
//! no cache padding, no dynamic allocation

ring_buffer! {
    storage = array,
    N = yes,
    arc = no,
    bip = no,
    contiguous = no,
    padded = no,
    pow2 = yes,
    module = "rtrb::embedded2",
    rb_doc = docstring!(
        /// ... array ... without padding ...
        ///
        /// TODO: ...
        ///
        /// *See also the [module-level documentation](crate::embedded2).*
    )
}
