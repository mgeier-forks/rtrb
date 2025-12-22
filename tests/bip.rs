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

// Different "slots" functions are tested separately and in all combinations (including no-op)
// because of caching and potential resetting of `skip`.
// Where possible, we test both write/read_chunk and push/pop.
#[rstest::rstest]
fn slots(
    #[values(
        |p: &mut Producer<i32>, one, two| {
            assert_eq!(p.slots(), one + two);
        },
        |p: &mut Producer<i32>, one, two| {
            assert_eq!(p.slots_contiguous(), (one, two));
        },
        |p: &mut Producer<i32>, one, _| {
            assert_eq!(p.slots_contiguous_first(), one);
        },
        |p: &mut Producer<i32>, one: usize, two| {
            assert!(p.write_chunk(one.max(two)).is_ok());
        },
        |_: &mut Producer<i32>, _, _| {},
    )]
    p_slots: fn(&mut Producer<i32>, usize, usize),
    #[values(
        |c: &mut Consumer<i32>, one, _| {
            assert_eq!(c.slots(), one);
        },
        |c: &mut Consumer<i32>, one, two| {
            assert_eq!(c.slots_contiguous(), (one, two));
        },
        |c: &mut Consumer<i32>, one, _| {
            assert_eq!(c.slots_contiguous_first(), one);
        },
        |c: &mut Consumer<i32>, one, _| {
            assert!(c.read_chunk(one).is_ok());
        },
        |_: &mut Consumer<i32>, _, _| {},
    )]
    c_slots: fn(&mut Consumer<i32>, usize, usize),
    #[values(
        |p: &mut Producer<i32>, slots| p.write_chunk(slots).unwrap().commit_all(),
        |p: &mut Producer<i32>, slots| for _ in 0..slots { assert!(p.push(0).is_ok()); },
    )]
    write_chunk_or_push: fn(p: &mut Producer<i32>, usize),
    #[values(
        |c: &mut Consumer<i32>, slots| c.read_chunk(slots).unwrap().commit_all(),
        |c: &mut Consumer<i32>, slots| for _ in 0..slots { assert!(c.pop().is_ok()); },
    )]
    read_chunk_or_pop: fn(c: &mut Consumer<i32>, usize),
) {
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

    // w: write index, r: read index, s: skip, x: data, _: empty

    // ₀_₁_₂_₃_₄_. w=0, r=0, s=_
    assert_slots!(5, 0; 0, 0);
    write_chunk_or_push(&mut p, 3);
    // ₀x₁x₂x₃_₄_. w=3, r=0, s=_
    assert_slots!(2, 0; 3, 0);
    read_chunk_or_pop(&mut c, 3);
    // ₀_₁_₂_₃_₄_. w=3, r=3, s=_
    assert_slots!(2, 3; 0, 0);
    // 2 slots are skipped:
    p.write_chunk(3).unwrap().commit_all();
    // ₀x₁x₂x₃_₄_. w=3, r=3->0, s=3
    // NB: w is allowed to overtake r, because r==s!
    assert_slots!(2, 0; 3, 0);

    // TODO: check both cases: (1) write and overtake r (followed by read); (2) read
}

// TODO: test if skipped elements are dropped
