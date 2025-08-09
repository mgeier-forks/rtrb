//! TODO: move this to crate::array once macros are complete.

storage_array! {
    arc = no,
    padded = yes,
    bip = no,
    rb_doc = "
Ring buffer using an array as storage.

TODO: ...

*See also the [module-level documentation](crate::array_temp).*
"
}

impl_everything_eventually! {
    N = yes,
    arc = no,
    bip = no,
    contiguous = no,
    pow2 = no,
    // TODO: update this:
    module = "rtrb::array_temp",
}
