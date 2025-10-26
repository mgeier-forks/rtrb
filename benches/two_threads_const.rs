macro_rules! create_two_threads_const_benchmark {
    ($($id:literal, $create:tt, $push:expr, $pop:expr,::)+) => {

use std::hint::black_box;
use criterion::{criterion_group, criterion_main, BenchmarkId};

fn help_with_type_inference<P, C, Push, Pop>(_: &P, _: &C, push: Push, pop: Pop) -> (Push, Pop)
where
    Push: Fn(&mut P, u8) -> bool,
    Pop: Fn(&mut C) -> Option<u8>,
{
    (push, pop)
}

#[allow(unused)]
fn criterion_benchmark(criterion: &mut criterion::Criterion) {
$(
    macro_rules! create $create
    let (mut p, mut c) = create!(8);
    let (push, pop) = help_with_type_inference(&p, &c, $push, $pop);
    // Just a quick check if the ring buffer works as expected:
    assert!(pop(&mut c).is_none());
    assert!(push(&mut p, 1));
    assert!(push(&mut p, 2));
    assert_eq!(pop(&mut c).unwrap(), 1);
    assert_eq!(pop(&mut c).unwrap(), 2);
    // NB: wrap-around differs between implementations (N vs. N-1 elements)
)+

    let mut group = criterion.benchmark_group("const-size");
    group.plot_config(criterion::PlotConfiguration::default()
        .summary_scale(criterion::AxisScale::Logarithmic));

$(
    macro_rules! add_bench {
        ($N:expr) => {
            group.bench_with_input(BenchmarkId::new($id, $N), &$N, |b, _| b.iter_custom(|iters| {
                macro_rules! create $create
                let (mut p, mut c) = create!($N);
                let (push, pop) = help_with_type_inference(&p, &c, $push, $pop);
                let push_thread = {
                    std::thread::spawn(move || {
                        // The timing starts once both threads are ready.
                        let start = std::time::Instant::now();
                        for i in 0..iters {
                            while !push(&mut p, black_box(i as u8)) {
                                std::hint::spin_loop();
                            }
                        }
                        start
                    })
                };
                // While the second thread is still starting up, this thread will busy-wait.
                for i in 0..iters {
                    loop {
                        if let Some(x) = pop(&mut c) {
                            assert_eq!(x, i as u8);
                            break;
                        }
                        std::hint::spin_loop();
                    }
                }
                // The timing stops once all items have been received.
                let stop = std::time::Instant::now();
                let start = push_thread.join().unwrap();
                stop.duration_since(start)
            }));
        };
    }

    add_bench!(4);
    add_bench!(8);
    add_bench!(16);
    add_bench!(32);
    add_bench!(64);
    add_bench!(128);
    add_bench!(256);
    add_bench!(512);
    add_bench!(1024);
    add_bench!(2048);
    add_bench!(4096);
    add_bench!(8192);
    add_bench!(16384);
    add_bench!(32768);
)+

    group.finish();
}

criterion_group!(benches, criterion_benchmark);
criterion_main!(benches);

    };
}

create_two_threads_const_benchmark! {
    "rtrb::array",
    { ($N:expr) => {{
        use rtrb::array::RingBuffer;
        static RB: RingBuffer<u8, $N> = RingBuffer::new();
        (RB.producer().unwrap(), RB.consumer().unwrap())
    }}},
    |p, i| p.push(i).is_ok(),
    |c| c.pop().ok(),
    ::
    "rtrb::array2",
    { ($N:expr) => {{
        use rtrb::array2::RingBuffer;
        static RB: RingBuffer<u8, $N> = RingBuffer::new();
        (RB.producer().unwrap(), RB.consumer().unwrap())
    }}},
    |p, i| p.push(i).is_ok(),
    |c| c.pop().ok(),
    ::
    "rtrb::bip_array",
    { ($N:expr) => {{
        use rtrb::bip_array::RingBuffer;
        static RB: RingBuffer<u8, $N> = RingBuffer::new();
        (RB.producer().unwrap(), RB.consumer().unwrap())
    }}},
    |p, i| p.push(i).is_ok(),
    |c| c.pop().ok(),
    ::
    "rtrb::arc_array",
    { ($N:expr) => {
        rtrb::arc_array::RingBuffer::<u8, $N>::new()
    }},
    |p, i| p.push(i).is_ok(),
    |c| c.pop().ok(),
    ::
    "rtrb::arc_array2",
    { ($N:expr) => {
        rtrb::arc_array2::RingBuffer::<u8, $N>::new()
    }},
    |p, i| p.push(i).is_ok(),
    |c| c.pop().ok(),
    ::
}
