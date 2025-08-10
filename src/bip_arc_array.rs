//! ...
//!
//! ... `static` can be used with `bip_array` ...
//!
//! ... maximum size limited by stack size ...

storage_array! {
    arc = yes,
    padded = yes,
    bip = yes,
    rb_doc = "
...

*See also the [module-level documentation](crate::bip_arc_array).*
"
}

impl_everything_eventually! {
    storage = array,
    N = yes,
    arc = yes,
    bip = yes,
    contiguous = yes,
    pow2 = no,
    module = "rtrb::bip_arc_array",
}
