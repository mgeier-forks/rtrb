//! ...
//!
//! no dynamic allocation, but cache-padded indices

storage_array! {
    arc = no,
    padded = yes,
    bip = no,
    rb_doc = "
Ring buffer using an array as storage.

TODO: ...

*See also the [module-level documentation](crate::array).*
"
}

impl_everything_eventually! {
    storage = array,
    N = yes,
    arc = no,
    bip = no,
    contiguous = no,
    pow2 = no,
    module = "rtrb::array",
}
