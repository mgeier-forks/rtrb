macro_rules! create_two_threads_array_benchmark {
    ($($id:literal, $create:expr, $push:expr, $pop:expr,)::+) => {

use criterion::{criterion_group, criterion_main};

fn help_with_type_inference<P, C, Create, Push, Pop>(create: Create, push: Push, pop: Pop) -> (Create, Push, Pop)
where
    Create: Fn() -> (P, C),
    Push: Fn(&mut P, u8) -> bool,
    Pop: Fn(&mut C) -> Option<u8>,
{
    (create, push, pop)
}

#[allow(unused)]
fn criterion_benchmark(criterion: &mut criterion::Criterion) {
$(
    let (create, push, pop) = help_with_type_inference($create, $push, $pop);
    // Just a quick check if the ring buffer works as expected:
    let (mut p, mut c) = create();
    assert!(pop(&mut c).is_none());
    assert!(push(&mut p, 1));
    assert!(push(&mut p, 2));
    assert_eq!(pop(&mut c).unwrap(), 1);
    assert_eq!(pop(&mut c).unwrap(), 2);
    // NB: wrap-around differs between implementations (N vs. N-1 elements)
)+

    let mut group = criterion.benchmark_group("two-threads-static");
    group.throughput(criterion::Throughput::Bytes(1));
$(
    group.bench_function($id, |b| {
        b.iter_custom(|iters| {
            let (create, push, pop) = help_with_type_inference($create, $push, $pop);
            // Queue is very short in order to force a lot of contention between threads.

            let (mut p, mut c) = create();

            let push_thread = {
                std::thread::spawn(move || {
                    // The timing starts once both threads are ready.
                    let start = std::time::Instant::now();
                    for i in 0..iters {
                        while !push(&mut p, i as u8) {
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
        });
    });
)+
    group.finish();
}

criterion_group!(benches, criterion_benchmark);
criterion_main!(benches);

    };
}

pub const SIZE: usize = 32;

create_two_threads_array_benchmark!(
    "rtrb::array",
    || {
        static RB: rtrb::array::RingBuffer<u8, SIZE> = rtrb::array::RingBuffer::new();
        (RB.producer().unwrap(), RB.consumer().unwrap())
    },
    |p, i| p.push(i).is_ok(),
    |c| c.pop().ok(),
    ::
    "static-rtrb2",
    || {
        static RB: rtrb::StaticRingBuffer2<u8, SIZE> = rtrb::StaticRingBuffer2::new();
        (RB.producer().unwrap(), RB.consumer().unwrap())
    },
    |p, i| p.push(i).is_ok(),
    |c| c.pop().ok(),
);
