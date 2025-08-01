//! A bi-partite ring buffer whose elements are stored in an [array].
//!
//! See [`rtrb::bip`](crate::bip) for a bi-partite ring buffer with dynamic storage.

use crate::{chunks::ChunkError, PopError, PushError};

storage_array! {
    padded = yes,
    bip = yes,
    rb_doc = "
Bi-partite ring buffer with elements stored in an [array].

TODO: some more docs, maybe links? [`RingBuffer::new()`].

*See also the [module-level documentation](crate::bip_array).*
"
}

impl_drop_all_elements! {
    bip = yes,
    N = (N)
}

impl_common! {
    N = (N)
}

impl_calculation! {
    pow2 = no,
    N = (N)
}

def_producer_consumer_ref! {
    bip = yes,
    N = (N)
}

impl_producer_consumer_common! {
    'a = ('a),
    N = (N)
}

impl_producer_consumer_bip! {
    'a = ('a),
    N = (N)
}

impl_next_head_bip! {
    'a = ('a),
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
