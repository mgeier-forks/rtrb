//! TODO: move this to crate::array once macros are complete.

storage_array! {
    padded = yes,
    partite = mop,
    rb_doc = "
Ring buffer using an array as storage.

TODO: ...

*See also the [module-level documentation](crate::array_temp).*
"
}

impl_partite! {
    partite = mop,
    N = N
}

impl_common! {
    N = N
}

impl_calculation! {
    pow2 = no,
    N = N
}
