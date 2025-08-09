//! A bi-partite ring buffer whose elements are stored in an [array].
//!
//! See [`rtrb::bip`](crate::bip) for a bi-partite ring buffer with dynamic storage.

storage_array! {
    arc = no,
    padded = yes,
    bip = yes,
    rb_doc = "
Bi-partite ring buffer with elements stored in an [array].

TODO: some more docs, maybe links? [`RingBuffer::new()`].

*See also the [module-level documentation](crate::bip_array).*
"
}

impl_everything_eventually! {
    N = yes,
    arc = no,
    bip = yes,
    contiguous = yes,
    pow2 = no,
    module = "rtrb::bip_array",
}
