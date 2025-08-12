//! A bi-partite ring buffer whose capacity is a power of two.

ring_buffer! {
    storage = vec,
    N = no,
    arc = yes,
    bip = yes,
    contiguous = yes,
    padded = yes,
    pow2 = yes,
    module = "rtrb::bip_arc2",
    rb_doc = docstring!(
        /// Bi-partite ring buffer.
        ///
        /// TODO: some more docs, maybe links? [`RingBuffer::new()`].
        ///
        /// *See also the [module-level documentation](crate::bip_arc2).*
    )
}
