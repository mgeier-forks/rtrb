//! TODO: move this to crate::array once macros are complete.

storage_array! {
    padded = yes,
    bip = no,
    rb_doc = "
Ring buffer using an array as storage.

TODO: ...

*See also the [module-level documentation](crate::array_temp).*
"
}

impl_partite! {
    bip = no,
    N = N
}

impl_common! {
    N = N
}

impl_calculation! {
    pow2 = no,
    N = N
}
