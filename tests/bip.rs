#![cfg(feature = "alloc")]

use rtrb::bip_arc::RingBuffer;

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

#[test]
fn slots() {
    // TODO: don't test slots_contiguous_max()?
    // TODO: test all slots functions separately because of caching?
    // TODO: test all slots functions twice in succession because of caching?
    // TODO: testing p/c slots at the same time should be fine
    // TODO: test without slots functions!

    // TODO: test write_chunk() which would skip, but then don't commit!
    // TODO: follow-up by multiple push(), show that `skip` is not set.
    let (mut p, mut c) = RingBuffer::<i32>::new(5);
    assert_eq!(p.slots(), 5);
    assert_eq!(p.slots_contiguous(), (5, 0));
    assert_eq!(p.slots_contiguous_first(), 5);
    assert_eq!(p.slots_contiguous_max(), 5);
    assert_eq!(c.slots(), 0);
    assert_eq!(c.slots_contiguous(), (0, 0));
    assert_eq!(c.slots_contiguous_first(), 0);

    p.write_chunk(3).unwrap().commit_all();
    assert_eq!(p.slots(), 2);
    assert_eq!(p.slots_contiguous(), (2, 0));
    assert_eq!(p.slots_contiguous_first(), 2);
    assert_eq!(p.slots_contiguous_max(), 2);
    assert_eq!(c.slots(), 3);
    assert_eq!(c.slots_contiguous(), (3, 0));
    assert_eq!(c.slots_contiguous_first(), 3);

    c.read_chunk(3).unwrap().commit_all();
    assert_eq!(p.slots(), 5);
    assert_eq!(p.slots_contiguous(), (2, 3));
    assert_eq!(p.slots_contiguous_first(), 2);
    assert_eq!(p.slots_contiguous_max(), 3);
    assert_eq!(c.slots(), 0);
    assert_eq!(c.slots_contiguous(), (0, 0));
    assert_eq!(c.slots_contiguous_first(), 0);

    p.write_chunk(3).unwrap().commit_all(); // 2 slots are skipped
    //assert_eq!(p.slots(), 2); // TODO: should be available, even though read index is still at pos. 4
    //assert_eq!(p.slots_contiguous(), (2, 0));
    //assert_eq!(p.slots_contiguous_first(), 2);
    //assert_eq!(p.slots_contiguous_max(), 2);
    //assert_eq!(c.slots(), 0);
    //assert_eq!(c.slots_contiguous(), (0, 0));
    //assert_eq!(c.slots_contiguous_first(), 0);
}

// TODO: test if skipped elements are dropped
