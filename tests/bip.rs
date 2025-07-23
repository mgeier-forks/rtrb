use rtrb::bip::RingBuffer;

#[test]
fn basic() {
    let (mut p, c) = RingBuffer::<i32>::new(7);
    if let Ok(chunk) = p.write_chunk_uninit(4) {
        // TODO
    } else {
        unreachable!();
    }
    assert!(p.write_chunk_uninit(4).is_err());
}
