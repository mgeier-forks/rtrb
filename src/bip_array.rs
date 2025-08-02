//! A bi-partite ring buffer whose elements are stored in an [array].
//!
//! See [`rtrb::bip`](crate::bip) for a bi-partite ring buffer with dynamic storage.

use crate::{PopError, PushError};

storage_array! {
    padded = yes,
    bip = yes,
    rb_doc = "
Bi-partite ring buffer with elements stored in an [array].

TODO: some more docs, maybe links? [`RingBuffer::new()`].

*See also the [module-level documentation](crate::bip_array).*
"
}

impl_everything_eventually! {
    bip = yes,
    contiguous = yes,
    pow2 = no,
    'a = ('a),
    N = (N)
}

def_producer_consumer_ref! {
    N = (N)
}

// "chunks" stuff.

impl_chunks_bip! {
    'a = ('a),
    N = (N)
}
impl_chunks_contiguous! {
    'a = ('a),
    N = (N)
}
impl_chunks_common! {
    N = (N)
}
