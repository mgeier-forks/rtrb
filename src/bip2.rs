//! A bi-partite ring buffer whose capacity is a power of two.

use crate::{
    chunks::ChunkError, diy::IS_ABANDONED, PeekError, PopError,
    PushError,
};

storage_vec! {
    padded = yes,
    bip = yes,
    rb_doc = "
Bi-partite ring buffer.

TODO: some more docs, maybe links? [`RingBuffer::new()`].

*See also the [module-level documentation](crate::bip).*
"
}

impl_drop_all_elements! {
    bip = yes,
    N = ()
}

impl_common! {
    N = ()
}

impl_calculation! {
    pow2 = yes,
    N = ()
}

// TODO: bip option?
def_producer_consumer_boxed! {}

def_boxed_ring_buffer! {}

impl_producer_consumer_common! {
    'a = (),
    N = ()
}

impl_producer_consumer_bip! {
    'a = (),
    N = ()
}

impl_next_head_bip! {
    'a = (),
    N = ()
}

// "chunks" stuff.

impl_chunks_bip! {
    'a = (),
    N = ()
}
impl_chunks_contiguous! {
    'a = (),
    N = ()
}
impl_chunks_common! {
    N = ()
}
