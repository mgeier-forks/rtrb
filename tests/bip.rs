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
    // different "slots" functions are tested separately and in all combinations (including no-op)
    // because of caching and potential resetting of `skip`.

    // TODO: follow-up by multiple push(), show that `skip` is not set.

    // NB: This iterates over the cartesian product:
    for (p_slots, c_slots) in C_SLOTS
        .iter()
        .flat_map(|cs| P_SLOTS.map(move |ps| (ps, cs)))
    {
        // TODO: another cartesian product with write_chunk_or_push() and read_chunk_or_pop()?

        let (mut p, mut c) = RingBuffer::new(5);

        macro_rules! assert_slots {
            ($p0:expr, $p1:expr; $c0:expr, $c1:expr) => {
                p_slots(&mut p, $p0, $p1);
                c_slots(&mut c, $c0, $c1);
                // repeat the same thing because caches might have been updated:
                p_slots(&mut p, $p0, $p1);
                c_slots(&mut c, $c0, $c1);
            };
        }

        // w: write index, r: read index, s: skip, x: data, _: empty, ( ): unable to write/read.

        // ₀_₁_₂_₃_₄_. w=0, r=0, s=_
        assert_slots!(5, 0; 0, 0);
        p.write_chunk(3).unwrap().commit_all();
        // ₀x₁x₂x₃_₄_. w=3, r=0, s=_
        assert_slots!(2, 0; 3, 0);
        c.read_chunk(3).unwrap().commit_all();
        // ₀_₁_₂_₃_₄_. w=3, r=3, s=_
        assert_slots!(2, 3; 0, 0);
        // 2 slots are skipped:
        p.write_chunk(3).unwrap().commit_all();
        // ₀x₁x₂x₃_₄_. w=3, r=3->0, s=3
        // NB: w is allowed to overtake r, because r==s!
        assert_slots!(2, 0; 3, 0);

        assert!(p.write_chunk(3).is_err());

        // TODO: check both cases: (1) write and overtake r (followed by read); (2) read
    }
}

// TODO: test if skipped elements are dropped
