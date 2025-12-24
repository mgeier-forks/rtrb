#![cfg(feature = "alloc")]

// TODO: try to reuse this for power-of-2 tests with bip_arc2?

use rtrb::bip_arc::{Consumer, Producer, RingBuffer};

type C<'a> = &'a mut Consumer<i32>;
type P<'a> = &'a mut Producer<i32>;

// Different "slots" functions are tested separately and in all combinations (including no-op)
// because of caching and potential resetting of `skip`.
// Where possible, we test both write/read_chunk and push/pop.
#[rstest::rstest]
fn slots(
    #[values(
        |p: P, x, y| assert_eq!(p.slots(), x + y),
        |p: P, x, y| assert_eq!(p.slots_contiguous(), (x, y)),
        |p: P, x, _| assert_eq!(p.slots_contiguous_first(), x),
        |p: P, x: usize, y| assert!(p.write_chunk(x.max(y)).is_ok()),
        |p: P, x: usize, y| assert!(p.write_chunk(x.max(y) + 1).is_err()),
        |_: P, _, _| {},
    )]
    p_slots: fn(P, usize, usize),
    #[values(
        |c: C, x, _| assert_eq!(c.slots(), x),
        |c: C, x, y| assert_eq!(c.slots_contiguous(), (x, y)),
        |c: C, x, _| assert_eq!(c.slots_contiguous_first(), x),
        |c: C, x, _| assert!(c.read_chunk(x).is_ok()),
        |c: C, x, _| assert!(c.read_chunk(x + 1).is_err()),
        |_: C, _, _| {},
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

    // w: write index, r: read index, s: skip, _: empty

    // ₀_₁_₂_₃_₄_. w=0, r=0, s=_
    assert_slots!(5, 0; 0, 0);
    write(&mut p, &[0, 1, 2]);
    // ₀0₁1₂2₃_₄_. w=3, r=0, s=_
    assert_slots!(2, 0; 3, 0);
    read(&mut c, &[0, 1, 2]);
    // ₀_₁_₂_₃_₄_. w=3, r=3, s=_
    assert_slots!(2, 3; 0, 0);
    // 2 slots are skipped:
    p.write_chunk(3)
        .map(|mut ch| {
            ch.as_mut_slice().copy_from_slice(&[9, 8, 7]);
            ch.commit_all();
        })
        .unwrap();
    // ₀9₁8₂7₃_₄_. w=3, r=3, s=3
    // NB: w is allowed to overtake r, because r==s!
    assert_slots!(2, 0; 3, 0);
    write(&mut p, &[6]);
    // ₀9₁8₂7₃6₄_. w=4, r=3/0, s=3
    assert_slots!(1, 0; 4, 0);
    write(&mut p, &[5]);
    // ₀9₁8₂7₃6₄5. w=0, r=3/0, s=(3)

    //assert_slots!(0, 0; 5, 0);

    //read(&mut c, &[9, 8, 7, 6]);
    // ₀_₁_₂_₃_₄5. w=0, r=4, s=_

    // TODO: check both cases: (1) write and overtake r (followed by read); (2) read
}

// TODO: test if skipped elements are dropped
// TODO: test if dropping directly after skipping works
