//! Single-threaded benchmarks, writing and reading chunks.
//!
//! Single-threaded usage is *not* a typical use case!

use std::hint::black_box;
use std::io::{Read, Write};

use criterion::{criterion_group, criterion_main};
use criterion::{AxisScale, PlotConfiguration};

use rtrb::{CopyToUninit, RingBuffer};

const CHUNK_SIZE: usize = 64;

fn add_function<F, M>(group: &mut criterion::BenchmarkGroup<M>, id: impl Into<String>, mut f: F)
where
    F: FnMut(&[u8]) -> [u8; CHUNK_SIZE],
    M: criterion::measurement::Measurement,
{
    #[allow(clippy::incompatible_msrv)]
    group.bench_function(id, |b| {
        let mut data = [0; CHUNK_SIZE];
        let mut i: u8 = 0;
        for dst in data.iter_mut() {
            *dst = i;
            i = i.wrapping_add(1);
        }
        b.iter(|| {
            assert_eq!(f(black_box(&data)), data);
        });
    });
}

pub fn criterion_benchmark(criterion: &mut criterion::Criterion) {
    let mut group = criterion.benchmark_group("single-thread-with-chunks");
    group.throughput(criterion::Throughput::Bytes(CHUNK_SIZE as u64));
    group.plot_config(PlotConfiguration::default().summary_scale(AxisScale::Logarithmic));

    let (mut p, mut c) = RingBuffer::<u8>::new(CHUNK_SIZE + 1);

    add_function(
        &mut group,
        "1-write_chunk_uninit+slice-read_chunk+slice",
        |data| {
            let mut result = [0; CHUNK_SIZE];
            let mut chunk = p.write_chunk_uninit(data.len()).unwrap();
            let (first, second) = chunk.as_mut_slices();
            let mid = first.len();
            data[..mid].copy_to_uninit(first);
            data[mid..].copy_to_uninit(second);
            unsafe {
                chunk.commit_all();
            }
            let chunk = c.read_chunk(data.len()).unwrap();
            let (first, second) = chunk.as_slices();
            let mid = first.len();
            result[..mid].copy_from_slice(first);
            result[mid..].copy_from_slice(second);
            chunk.commit_all();
            result
        },
    );

    add_function(&mut group, "2-write_chunk_uninit+slice-read", |data| {
        let mut result = [0; CHUNK_SIZE];
        let mut chunk = p.write_chunk_uninit(data.len()).unwrap();
        let (first, second) = chunk.as_mut_slices();
        let mid = first.len();
        data[..mid].copy_to_uninit(first);
        data[mid..].copy_to_uninit(second);
        unsafe {
            chunk.commit_all();
        }
        let _ = c.read(&mut result).unwrap();
        result
    });

    add_function(&mut group, "3-write_chunk+slice-read_chunk+slice", |data| {
        let mut result = [0; CHUNK_SIZE];
        let mut chunk = p.write_chunk(data.len()).unwrap();
        let (first, second) = chunk.as_mut_slices();
        let mid = first.len();
        first.copy_from_slice(&data[..mid]);
        second.copy_from_slice(&data[mid..]);
        chunk.commit_all();
        let chunk = c.read_chunk(data.len()).unwrap();
        let (first, second) = chunk.as_slices();
        let mid = first.len();
        result[..mid].copy_from_slice(first);
        result[mid..].copy_from_slice(second);
        chunk.commit_all();
        result
    });

    add_function(&mut group, "4-write_chunk+slice-read", |data| {
        let mut result = [0; CHUNK_SIZE];
        let mut chunk = p.write_chunk(data.len()).unwrap();
        let (first, second) = chunk.as_mut_slices();
        let mid = first.len();
        first.copy_from_slice(&data[..mid]);
        second.copy_from_slice(&data[mid..]);
        chunk.commit_all();
        let _ = c.read(&mut result).unwrap();
        result
    });

    add_function(&mut group, "5-write-read_chunk+slice", |data| {
        let mut result = [0; CHUNK_SIZE];
        let _ = p.write(data).unwrap();
        let chunk = c.read_chunk(data.len()).unwrap();
        let (first, second) = chunk.as_slices();
        let mid = first.len();
        result[..mid].copy_from_slice(first);
        result[mid..].copy_from_slice(second);
        chunk.commit_all();
        result
    });

    add_function(&mut group, "6-write-read", |data| {
        let mut result = [0; CHUNK_SIZE];
        let _ = p.write(data).unwrap();
        let _ = c.read(&mut result).unwrap();
        result
    });

    add_function(&mut group, "7-push_slice-pop_slice", |data| {
        let mut result = [0; CHUNK_SIZE];
        let _ = p.push_slice(data);
        let _ = c.pop_slice(&mut result);
        result
    });

    add_function(&mut group, "7-push_slice2-pop_slice", |data| {
        let mut result = [0; CHUNK_SIZE];
        let _ = p.push_slice2(data);
        let _ = c.pop_slice(&mut result);
        result
    });

    add_function(&mut group, "7-push_slice3-pop_slice", |data| {
        let mut result = [0; CHUNK_SIZE];
        let _ = p.push_slice3(data);
        let _ = c.pop_slice(&mut result);
        result
    });

    add_function(&mut group, "7-push_slice_using_entire-pop_slice", |data| {
        let mut result = [0; CHUNK_SIZE];
        let _ = p.push_slice_using_entire(data);
        let _ = c.pop_slice(&mut result);
        result
    });

    add_function(&mut group, "7-push_slice_using_entire2-pop_slice", |data| {
        let mut result = [0; CHUNK_SIZE];
        let _ = p.push_slice_using_entire2(data);
        let _ = c.pop_slice(&mut result);
        result
    });

    add_function(&mut group, "7-push_slice_using_entire3-pop_slice", |data| {
        let mut result = [0; CHUNK_SIZE];
        let _ = p.push_slice_using_entire3(data);
        let _ = c.pop_slice(&mut result);
        result
    });

    add_function(&mut group, "8-write_chunk_uninit+iter-read", |data| {
        let mut result = [0; CHUNK_SIZE];
        let chunk = p.write_chunk_uninit(data.len()).unwrap();
        chunk.fill_from_iter(&mut data.iter().copied());
        let _ = c.read(&mut result).unwrap();
        result
    });

    add_function(&mut group, "9-write-read_chunk+iter", |data| {
        let mut result = [0; CHUNK_SIZE];
        let _ = p.write(data).unwrap();
        let chunk = c.read_chunk(data.len()).unwrap();
        for (dst, src) in result.iter_mut().zip(chunk) {
            *dst = src;
        }
        result
    });

    group.finish();
}

criterion_group!(benches, criterion_benchmark);
criterion_main!(benches);
