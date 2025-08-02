use rtrb::bip::RingBuffer;

#[test]
fn basic() {
    let (mut p, _c) = RingBuffer::<i32>::new(7);
    if let Ok(chunk) = p.write_chunk(4) {
        chunk.commit_all();
    } else {
        unreachable!();
    }
    assert!(p.write_chunk_uninit(4).is_err());
}

// TODO: test if skipped elements are dropped
