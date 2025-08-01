//! TODO: move this to crate::array once macros are complete.

use crate::{chunks::ChunkError, PopError, PushError};

storage_array! {
    padded = yes,
    bip = no,
    rb_doc = "
Ring buffer using an array as storage.

TODO: ...

*See also the [module-level documentation](crate::array_temp).*
"
}

impl_drop_all_elements! {
    bip = no,
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
    bip = no,
    N = (N)
}

impl_producer_consumer_common! {
    'a = ('a),
    N = (N)
}

impl_next_head_non_bip! {
    'a = ('a),
    N = (N)
}

// "chunks" stuff.

impl_chunks_non_bip! {
    'a = ('a),
    N = (N)
}

impl_chunks_mop! {
    'a = ('a),
    N = (N)
}

impl_chunks_non_contiguous! {
    'a = ('a),
    N = (N)
}

impl_chunks_common! {
    N = (N)
}
