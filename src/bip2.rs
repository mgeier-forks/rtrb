//! A bi-partite ring buffer whose capacity is a power of two.

storage_vec! {
    padded = yes,
    bip = yes,
    rb_doc = "
Bi-partite ring buffer.

TODO: some more docs, maybe links? [`RingBuffer::new()`].

*See also the [module-level documentation](crate::bip).*
"
}

impl_everything_eventually! {
    N = no,
    arc = yes,
    bip = yes,
    contiguous = yes,
    pow2 = yes,
    module = "rtrb::bip2",
}
