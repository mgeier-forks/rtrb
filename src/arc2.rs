//! ...

ring_buffer! {
    storage = vec,
    N = no,
    arc = yes,
    bip = no,
    contiguous = no,
    padded = yes,
    pow2 = yes,
    module = "rtrb::arc2",
    rb_doc = docstring!(
        /// Ring buffer ...
        ///
        /// TODO: ...
        ///
        /// *See also the [module-level documentation](crate::arc2).*
    )
}
