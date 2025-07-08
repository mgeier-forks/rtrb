#![cfg(feature = "mmap")]

use rtrb::mmap::MmapRingBuffer;

#[test]
fn basic() {
    let (mut p, mut c) = MmapRingBuffer::<usize>::new(1);
    println!("capacity: {}", p.capacity());
    for round in 0..3 {
        println!("round {round}");
        for i in 0..p.capacity() {
            p.push(i).unwrap();
        }
        assert!(p.push(42).is_err());
        for i in 0..p.capacity() {
            let v = c.pop().unwrap();
            assert_eq!(v, i);
        }
        assert!(c.pop().is_err());
    }
    // TODO: try chunks!
}
