#![cfg(feature = "alloc")]

// TODO: try to reuse this for power-of-2 tests with bip_arc2?

use rtrb::bip_arc::{Consumer, Producer, RingBuffer};

type C<'a> = &'a mut Consumer<i32>;
type P<'a> = &'a mut Producer<i32>;

macro_rules! assert_eq_stringify {
    ($tested:expr, $expected:expr) => {
        if !($tested == $expected) {
            panic!(
                "assertion `{} == {:?}` failed, wrong value: {:?}",
                stringify!($tested),
                $expected,
                $tested
            );
        }
    };
}

// Different "slots" functions are tested separately and in all combinations
// (including a no-op) because of caching.
// Where possible, we test both write/read_chunk and push/pop.
#[rstest::rstest]
fn slots(
    // NB: None of these functions will reset `skip`:
    #[values(
        |p: P, x, y| assert_eq_stringify!(p.slots(), x + y),
        |p: P, x, y| assert_eq_stringify!(p.slots_contiguous(), (x, y)),
        |p: P, x, _| assert_eq_stringify!(p.slots_contiguous_first(), x),
        |p: P, x: usize, y| assert!(p.write_chunk(x.max(y)).is_ok()),
        |p: P, x: usize, y| assert!(p.write_chunk(x.max(y) + 1).is_err()),
        |_: P, _, _| {},
    )]
    p_slots: fn(P, usize, usize),
    // NB: All these functions are resetting `skip` if `head == skip`:
    #[values(
        |c: C, x, y| assert_eq_stringify!(c.slots(), x + y),
        |c: C, x, y| assert_eq_stringify!(c.slots_contiguous(), (x, y)),
        |c: C, x, _| assert_eq_stringify!(c.slots_contiguous_first(), x),
        |c: C, x, _| assert!(c.read_chunk(x).is_ok()),
        |c: C, x, _| assert!(c.read_chunk(x + 1).is_err()),
        |c: C, x, _| assert_eq!(c.peek().is_ok(), x > 0),
        |c: C, x, _| assert_eq!(c.is_empty(), x == 0),
    )]
    c_slots: fn(C, usize, usize),
    #[values(
        |p: P, vs: &[_]| p.write_chunk(vs.len()).map(|mut ch| {
            ch.as_mut_slice().copy_from_slice(vs);
            ch.commit_all();
        }).unwrap(),
        |p: P, vs: &[_]| for &v in vs { assert!(p.push(v).is_ok()); },
    )]
    write: fn(P, &[i32]),
    #[values(
        |c: C, vs: &[_]| c.read_chunk(vs.len()).map(|ch| {
            assert_eq!(ch.as_slice(), vs);
            ch.commit_all();
        }).unwrap(),
        |c: C, vs: &[_]| for &v in vs { assert_eq!(c.pop(), Ok(v)); },
    )]
    read: fn(C, &[i32]),
) {
    let (mut p, mut c) = RingBuffer::new(8);
    let p = &mut p;
    let c = &mut c;

    macro_rules! assert_p_slots {
        ($p0:expr, $p1:expr) => {
            p_slots(p, $p0, $p1);
            // repeat the same thing because caches might have been updated:
            p_slots(p, $p0, $p1);
        };
    }

    macro_rules! assert_c_slots {
        ($c0:expr, $c1:expr) => {
            c_slots(c, $c0, $c1);
            // repeat the same thing because caches might have been updated:
            c_slots(c, $c0, $c1);
        };
    }

    macro_rules! assert_slots {
        ($p0:expr, $p1:expr; $c0:expr, $c1:expr) => {
            p_slots(p, $p0, $p1);
            c_slots(c, $c0, $c1);
            // repeat the same thing because caches might have been updated:
            p_slots(p, $p0, $p1);
            c_slots(c, $c0, $c1);
        };
    }

    // w: write index, r: read index, s: skip, _: empty

    // ₀_₁_₂_₃_₄_₅_₆_₇_. w=0, r=0, s=_
    assert_slots!(8, 0; 0, 0);
    write(p, &[0, 1, 2, 3, 4]);
    // ₀0₁1₂2₃3₄4₅_₆_₇_. w=5, r=0, s=_
    assert_slots!(3, 0; 5, 0);
    read(c, &[0, 1, 2, 3, 4]);
    // ₀_₁_₂_₃_₄_₅_₆_₇_. w=5, r=5, s=_
    assert_slots!(3, 5; 0, 0);
    // 3 slots are skipped:
    p.write_chunk(4)
        .map(|mut ch| {
            ch.as_mut_slice().copy_from_slice(&[9, 8, 7, 6]);
            ch.commit_all();
        })
        .unwrap();
    // ₀9₁8₂7₃6₄_₅_₆_₇_. w=4, r=5, s=5
    // Even though 4 slots are empty, only one can be written right now ...
    assert_p_slots!(1, 0);
    // ... but any consumer operation will reset r&s ...
    assert_c_slots!(4, 0);
    // ... and now all 4 slots are available for writing.
    // ₀9₁8₂7₃6₄_₅_₆_₇_. w=4, r=0, s=_
    assert_p_slots!(4, 0);
    read(c, &[9, 8, 7]);
    // ₀_₁_₂_₃6₄_₅_₆_₇_. w=4, r=3, s=_
    assert_slots!(4, 3; 1, 0);
    write(p, &[5, 4]);
    // ₀_₁_₂_₃6₄5₅4₆_₇_. w=6, r=3, s=_
    assert_slots!(2, 3; 3, 0);
    // 2 slots are skipped:
    p.write_chunk(3)
        .map(|mut ch| {
            ch.as_mut_slice().copy_from_slice(&[3, 2, 1]);
            ch.commit_all();
        })
        .unwrap();
    // ₀3₁2₂1₃6₄5₅4₆_₇_. w=3, r=3, s=6
    assert_slots!(0, 0; 3, 3);
    assert!(p.is_full());
    read(c, &[6, 5]);
    // ₀3₁2₂1₃_₄_₅4₆_₇_. w=3, r=5, s=6
    assert_slots!(2, 0; 1, 3);
    // Reading [3, 2, 1] is not possible, [4] has to be read first.
    assert!(c.read_chunk(3).is_err());
    read(c, &[4]);
    // ₀3₁2₂1₃_₄_₅_₆_₇_. w=3, r=6, s=6
    // Even though the slots are empty, we cannot write beyond r ...
    assert_p_slots!(3, 0);
    // ... but any consumer operation will reset r&s ...
    assert_c_slots!(3, 0);
    // ... and now all empty slots are available for writing.
    // ₀3₁2₂1₃_₄_₅_₆_₇_. w=3, r=0, s=_
    assert_p_slots!(5, 0);
}

// TODO: test if skipped elements are dropped
// TODO: test if dropping directly after skipping works
