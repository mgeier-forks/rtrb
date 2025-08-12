//! ...
//!
//! ... `static` can be used with `bip_array` ...
//!
//! ... maximum size limited by stack size ...

ring_buffer! {
    storage = array,
    N = yes,
    arc = yes,
    bip = yes,
    contiguous = yes,
    padded = yes,
    pow2 = no,
    module = "rtrb::bip_arc_array",
    rb_doc = docstring!(
        /// ...
        ///
        /// *See also the [module-level documentation](crate::bip_arc_array).*
    )
}
