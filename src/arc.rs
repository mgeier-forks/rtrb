//! ...

storage_array! {
    arc = yes,
    padded = yes,
    bip = no,
    rb_doc = "
Ring buffer ...

TODO: ...

*See also the [module-level documentation](crate::arc).*
"
}

impl_everything_eventually! {
    storage = vec,
    N = yes,
    arc = yes,
    bip = no,
    contiguous = no,
    pow2 = no,
    module = "rtrb::arc",
}
