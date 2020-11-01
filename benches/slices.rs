use criterion::black_box;
use criterion::{criterion_group, criterion_main};
use rtrb::RingBuffer;

pub fn slices(criterion: &mut criterion::Criterion) {
    criterion.bench_function("pop-single-element-slices", |b| {
        let (mut p, mut c) = RingBuffer::<u8>::new(1).split();
        let mut i = 0;
        b.iter(|| {
            p.push(black_box(i)).unwrap();
            if let Ok(slices) = c.pop_slices(1) {
                assert_eq!(slices.first[0], black_box(i));
            } else {
                unreachable!();
            }
            i = i.wrapping_add(black_box(1));
        })
    });

    criterion.bench_function("push-single-element-slices", |b| {
        let (mut p, mut c) = RingBuffer::<u8>::new(1).split();
        let mut i = 0;
        b.iter(|| {
            if let Ok(n) = p.push_slices(1, |first, _second| {
                first[0] = black_box(i);
                1
            }) {
                debug_assert_eq!(n, 1);
            } else {
                unreachable!();
            }
            assert_eq!(c.pop(), Ok(black_box(i)));
            i = i.wrapping_add(black_box(1));
        })
    });
}

criterion_group!(benches, slices);
criterion_main!(benches);
