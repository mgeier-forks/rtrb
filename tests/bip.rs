#![cfg(feature = "alloc")]

use rtrb::bip_arc::{Consumer, Producer, RingBuffer};

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

const P_SLOTS: [fn(&mut Producer<i32>, usize, usize); 5] = [
    |p, one, two| {
        assert_eq!(p.slots(), one + two);
    },
    |p, one, two| {
        assert_eq!(p.slots_contiguous(), (one, two));
    },
    |p, one, _| {
        assert_eq!(p.slots_contiguous_first(), one);
    },
    |p, one, two| {
        assert!(p.write_chunk(one.max(two)).is_ok());
    },
    |_, _, _| {},
];

const C_SLOTS: [fn(&mut Consumer<i32>, usize, usize); 5] = [
    |c, one, _| {
        assert_eq!(c.slots(), one);
    },
    |c, one, two| {
        assert_eq!(c.slots_contiguous(), (one, two));
    },
    |c, one, _| {
        assert_eq!(c.slots_contiguous_first(), one);
    },
    |c, one, _| {
        assert!(c.read_chunk(one).is_ok());
    },
    |_, _, _| {},
];

#[test]
fn slots() {
    // different "slots" functions are tested separately because of caching.

    // TODO: follow-up by multiple push(), show that `skip` is not set.
    for (p_slots, c_slots) in P_SLOTS.iter().zip(C_SLOTS) {
        let (mut p, mut c) = RingBuffer::<i32>::new(5);

        macro_rules! assert_slots {
            ($p0:expr, $p1:expr; $c0:expr, $c1:expr) => {
                p_slots(&mut p, $p0, $p1);
                c_slots(&mut c, $c0, $c1);
                // repeat the same thing because caches might have been updated:
                p_slots(&mut p, $p0, $p1);
                c_slots(&mut c, $c0, $c1);
            };
        }

        assert_slots!(5, 0; 0, 0);
        p.write_chunk(3).unwrap().commit_all();
        assert_slots!(2, 0; 3, 0);
        c.read_chunk(3).unwrap().commit_all();
        assert_slots!(2, 3; 0, 0);
        p.write_chunk(3).unwrap().commit_all(); // 2 slots are skipped
        // The read index is 2, but writing is still possible, because `skip` is also 2!
        //assert_slots!(2, 0; 0, 0); // TODO: should be available, even though read index is still at pos. 4
    }
}

// TODO: test if skipped elements are dropped
