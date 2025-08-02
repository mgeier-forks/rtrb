//! TODO: move this to crate::array once macros are complete.

use crate::{PopError, PushError};

storage_array! {
    padded = yes,
    bip = no,
    rb_doc = "
Ring buffer using an array as storage.

TODO: ...

*See also the [module-level documentation](crate::array_temp).*
"
}

impl_everything_eventually! {
    bip = no,
    contiguous = no,
    pow2 = no,
    'a = ('a),
    N = (N)
}

def_producer_consumer_ref! {
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
