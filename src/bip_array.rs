//! A bi-partite ring buffer whose elements are stored in an [array].
//!
//! See [`rtrb::bip_arc`](crate::bip_arc) for a bi-partite ring buffer with dynamic storage.

ring_buffer! {
    storage = array,
    N = yes,
    arc = no,
    bip = yes,
    contiguous = yes,
    padded = yes,
    pow2 = no,
    module = "rtrb::bip_array",
    rb_doc = docstring!(
        /// Bi-partite ring buffer with elements stored in an [array].
        ///
        /// TODO: some more docs, maybe links? [`RingBuffer::new()`].
        ///
        /// *See also the [module-level documentation](crate::bip_array).*
    )
}
