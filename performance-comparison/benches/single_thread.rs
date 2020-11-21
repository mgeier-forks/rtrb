#[allow(dead_code)]
#[path = "../../benches/single_thread_with_chunks.rs"]
mod single_thread_with_chunks;

use single_thread_with_chunks::{add_function, CHUNKS, CHUNK_SIZE};

use criterion::{criterion_group, criterion_main};
use criterion::{AxisScale, PlotConfiguration};

fn criterion_benchmark(criterion: &mut criterion::Criterion) {
    let mut group = criterion.benchmark_group("single-thread");
    group.throughput(criterion::Throughput::Bytes(CHUNK_SIZE as u64));
    group.plot_config(PlotConfiguration::default().summary_scale(AxisScale::Logarithmic));

    {
        use std::io::{Read, Write};
        let (mut p, mut c) = rtrb::RingBuffer::<u8>::with_chunks(CHUNKS, CHUNK_SIZE).split();
        add_function(&mut group, "rtrb", |data| {
            let mut result = [0; CHUNK_SIZE];
            let _ = p.write(&data).unwrap();
            let _ = c.read(&mut result).unwrap();
            result
        });
    }

    {
        let (mut p, mut c) = ringbuf::RingBuffer::<u8>::new(CHUNKS * CHUNK_SIZE).split();
        add_function(&mut group, "ringbuf", |data| {
            let mut result = [0; CHUNK_SIZE];
            let _ = p.push_slice(&data);
            let _ = c.pop_slice(&mut result);
            result
        });
    }

    {
        let (producer, consumer) =
            npnc::bounded::spsc::channel((CHUNKS * CHUNK_SIZE).next_power_of_two());
        add_function(&mut group, "npnc", |data| {
            let mut result = [0; CHUNK_SIZE];
            for &src in data {
                producer.produce(src).unwrap();
            }
            for dst in &mut result {
                *dst = consumer.consume().unwrap();
            }
            result
        });
    }

    {
        let ringbuf = jack::RingBuffer::new(CHUNKS * CHUNK_SIZE).unwrap();
        let (mut reader, mut writer) = ringbuf.into_reader_writer();
        add_function(&mut group, "jack", |data| {
            let mut result = [0; CHUNK_SIZE];
            let _ = writer.write_buffer(&data);
            let _ = reader.read_buffer(&mut result);
            result
        });
    }

    group.finish();
}

criterion_group!(benches, criterion_benchmark);
criterion_main!(benches);
