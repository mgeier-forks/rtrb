//! ...

ring_buffer! {
    storage = vec,
    N = no,
    arc = yes,
    bip = no,
    contiguous = no,
    padded = yes,
    pow2 = no,
    module = "rtrb::arc",
    rb_doc = docstring!(
        /// Ring buffer ...
        ///
        /// TODO: ...
        ///
        /// TODO: move generic description into macro:
        ///
        /// A bounded single-producer single-consumer (SPSC) queue.
        ///
        /// Elements can be written with a [`Producer`] and read with a [`Consumer`],
        /// both of which can be obtained with [`RingBuffer::new()`].
        ///
        /// *See also the [module-level documentation](crate::arc).*
    )
}
