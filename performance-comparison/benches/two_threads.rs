#[path = "../../benches/two_threads_single_byte.rs"]
mod two_threads_single_byte;

use two_threads_single_byte::{add_function, CAPACITY};

use criterion::{criterion_group, criterion_main};
use criterion::{AxisScale, PlotConfiguration};

fn criterion_benchmark(criterion: &mut criterion::Criterion) {
    let mut group = criterion.benchmark_group("two-threads");
    group.throughput(criterion::Throughput::Bytes(1));
    group.plot_config(PlotConfiguration::default().summary_scale(AxisScale::Logarithmic));

    add_function(
        &mut group,
        "rtrb-",
        || rtrb::RingBuffer::<u8>::new(CAPACITY).split(),
        |p, i| p.push(i).is_ok(),
        |c| c.pop().ok(),
    );

    add_function(
        &mut group,
        "jack-",
        || {
            let ringbuf = jack::RingBuffer::new(CAPACITY).unwrap();
            let (reader, writer) = ringbuf.into_reader_writer();
            (writer, reader)
        },
        |w, i| w.write_buffer(&[i]) == 1,
        |r| {
            let mut buf = [0];
            if r.read_buffer(&mut buf) == 1 {
                Some(buf[0])
            } else {
                None
            }
        },
    );

    add_function(
        &mut group,
        "ringbuf-",
        || ringbuf::RingBuffer::<u8>::new(CAPACITY).split(),
        |p, i| p.push(i).is_ok(),
        |c| c.pop(),
    );

    add_function(
        &mut group,
        "npnc-",
        || npnc::bounded::spsc::channel(CAPACITY.next_power_of_two()),
        |p, i| p.produce(i).is_ok(),
        |c| c.consume().ok(),
    );

    group.finish();
}

criterion_group!(benches, criterion_benchmark);
criterion_main!(benches);
