//! Ring buffer, array, power of 2.

storage_array! {
    arc = no,
    padded = yes,
    bip = no,
    rb_doc = "
Ring buffer using an array with a power-of-two size as storage.

*See also the [module-level documentation](crate::array2).*
"
}

impl_everything_eventually! {
    storage = array,
    N = yes,
    arc = no,
    bip = no,
    contiguous = no,
    pow2 = yes,
    module = "rtrb::array2",
}
