#![cfg(feature = "mmap")]

use rtrb::mmap::RingBuffer;

#[test]
fn basic() {
    let (mut p, mut c) = RingBuffer::<usize>::new(1);
    // Capacity has been increased to multiple of page size / size of `T`.
    assert!(p.capacity() > 1);
    let half = p.capacity() / 2;
    for i in 0..half {
        p.push(i).unwrap();
        let v = c.pop().unwrap();
        assert_eq!(v, i);
    }
    // TODO: use write_chunk() instead of push()
    for i in half..half + p.capacity() {
        p.push(i).unwrap();
    }
    assert!(p.push(42).is_err());
    if let Ok(chunk) = c.read_chunk(c.capacity()) {
        for i in 0..p.capacity() {
            assert_eq!(half + i, chunk.as_slice()[i]);
        }
        chunk.commit_all();
    } else {
        unreachable!()
    }
    assert!(c.pop().is_err());
}
