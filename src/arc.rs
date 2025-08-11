//! ...

storage_vec! {
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
    N = no,
    arc = yes,
    bip = no,
    contiguous = no,
    pow2 = no,
    module = "rtrb::arc",
}
