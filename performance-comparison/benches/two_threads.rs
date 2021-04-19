#[path = "../../benches/two_threads.rs"]
mod two_threads;

use two_threads::add_function;

use criterion::{criterion_group, criterion_main};

fn criterion_benchmark(criterion: &mut criterion::Criterion) {
    let mut group = criterion.benchmark_group("two-threads");

    add_function(
        &mut group,
        "-npnc",
        |capacity| npnc::bounded::spsc::channel(capacity.next_power_of_two()),
        |p, i| p.produce(i).is_ok(),
        |c| c.consume().ok(),
    );

    add_function(
        &mut group,
        "-ringbuf",
        |capacity| ringbuf::RingBuffer::new(capacity).split(),
        |p, i| p.push(i).is_ok(),
        |c| c.pop(),
    );

    add_function(
        &mut group,
        "-rtrb",
        |capacity| rtrb::RingBuffer::new(capacity).split(),
        |p, i| p.push(i).is_ok(),
        |c| c.pop().ok(),
    );

    group.finish();
}

criterion_group!(benches, criterion_benchmark);
criterion_main!(benches);
