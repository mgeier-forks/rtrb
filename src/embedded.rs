//! ...
//!
//! no cache padding, no dynamic allocation
//! power-of-two optimizations might be done automatically by the compiler? TODO: verify

ring_buffer! {
    storage = array,
    N = yes,
    arc = no,
    bip = no,
    contiguous = no,
    padded = no,
    pow2 = no,
    module = "rtrb::embedded",
    rb_doc = docstring!(
        /// ... array ... without padding ...
        ///
        /// TODO: ...
        ///
        /// *See also the [module-level documentation](crate::embedded).*
    )
}
