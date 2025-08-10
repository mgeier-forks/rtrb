macro_rules! create_bip_benchmark {
    ($($id:literal, $create:expr, $write_chunk:expr, $read_chunk:expr,::)+) => {

use std::convert::TryFrom as _;
use std::sync::{Arc, Barrier};

use criterion::{black_box, criterion_group, criterion_main};

fn help_with_type_inference<P, C, Create, WriteChunk, ReadChunk>(create: Create, write_chunk: WriteChunk, read_chunk: ReadChunk) -> (Create, WriteChunk, ReadChunk)
where
    Create: Fn(usize) -> (P, C),
    WriteChunk: Fn(&mut P, &[u8]) -> bool,
    ReadChunk: Fn(&mut C, &mut[u8]) -> bool,
{
    (create, write_chunk, read_chunk)
}

#[allow(unused)]
fn criterion_benchmark(criterion: &mut criterion::Criterion) {
$(
    let (create, write_chunk, read_chunk) = help_with_type_inference($create, $write_chunk, $read_chunk);
    // Just a quick check if the ring buffer works as expected:
    let (mut p, mut c) = create(2);
    let mut a = [0u8; 1];
    assert!(!read_chunk(&mut c, &mut a));
    assert!(write_chunk(&mut p, &[10]));
    assert!(!write_chunk(&mut p, &[20, 30]));
    assert!(write_chunk(&mut p, &[40]));
    assert!(!write_chunk(&mut p, &[50]));
    assert!(read_chunk(&mut c, &mut a));
    assert_eq!(a, [10]);
    assert!(read_chunk(&mut c, &mut a));
    assert_eq!(a, [40]);
    assert!(!read_chunk(&mut c, &mut a));
)+

    let mut group_large = criterion.benchmark_group("large");
    group_large.throughput(criterion::Throughput::Bytes(1));
$(
    group_large.bench_function($id, |b| {
        b.iter_custom(|iters| {
            let (create, write_chunk, read_chunk) = help_with_type_inference($create, $write_chunk, $read_chunk);
            let iters = usize::try_from(iters).unwrap();
            // Queue is so long that there is no contention between threads.
            let size = (3 * iters).next_power_of_two();
            let (mut p, mut c) = create(size);
            let mut dummy = vec![0; size - iters];
            assert!(write_chunk(&mut p, &dummy));
            assert!(read_chunk(&mut c, &mut dummy));
            let half = iters / 2;
            let data: Vec<_> = (0..(iters + half)).map(|x| x as u8).collect();
            assert!(write_chunk(&mut p, &data[..half]));
            // NB: This forces a "skip" over the last `half` slots:
            assert!(write_chunk(&mut p, &data[half..]));
            let barrier = Arc::new(Barrier::new(3));
            let push_thread = {
                let barrier = Arc::clone(&barrier);
                std::thread::spawn(move || {
                    barrier.wait();
                    let start_pushing = std::time::Instant::now();
                    for i in 0..iters {
                        assert!(write_chunk(&mut p, black_box(&[i as u8])));
                    }
                    let stop_pushing = std::time::Instant::now();
                    (start_pushing, stop_pushing)
                })
            };
            let trigger_thread = {
                let barrier = Arc::clone(&barrier);
                std::thread::spawn(move || {
                    // Try to force other threads to go to sleep on barrier.
                    std::thread::yield_now();
                    std::thread::yield_now();
                    std::thread::yield_now();
                    barrier.wait();
                    // Hopefully, the other two threads now wake up at the same time.
                })
            };
            barrier.wait();
            let start_popping = std::time::Instant::now();
            for i in 0..iters {
                let mut a = [0];
                assert!(read_chunk(&mut c, black_box(&mut a)));
                assert_eq!(a, [i as u8]);
            }
            let stop_popping = std::time::Instant::now();
            let (start_pushing, stop_pushing) = push_thread.join().unwrap();
            trigger_thread.join().unwrap();
            let total = stop_pushing
                .max(stop_popping)
                .duration_since(start_pushing.min(start_popping));

            /*
            if start_pushing < start_popping {
                println!(
                    "popping started {:?} after pushing",
                    start_popping.duration_since(start_pushing)
                );
            } else {
                println!(
                    "pushing started {:?} after popping",
                    start_pushing.duration_since(start_popping)
                );
            }
            */

            // The goal is that both threads are finished at around the same time.
            // This can be checked with the following output.
            /*
            if stop_pushing < stop_popping {
                let diff = stop_popping.duration_since(stop_pushing);
                println!(
                    "popping stopped {diff:?} after pushing ({:.1}% of total time)",
                    (diff.as_secs_f64() / total.as_secs_f64()) * 100.0
                );
            } else {
                let diff = stop_pushing.duration_since(stop_popping);
                println!(
                    "pushing stopped {diff:?} after popping ({:.1}% of total time)",
                    (diff.as_secs_f64() / total.as_secs_f64()) * 100.0
                );
            }
            */

            // This is not part of the timing measurements, just checking if the pushed data is OK.
            assert!(read_chunk(&mut c, &mut dummy[iters..(iters + half)]));
            assert!(read_chunk(&mut c, &mut dummy[..iters]));
            assert_eq!(&dummy[..iters + half], &data);
            // The queue is empty.
            assert!(!read_chunk(&mut c, &mut dummy[..1]));
            total
        });
    });
)+
    group_large.finish();

    let mut group_small = criterion.benchmark_group("small");
    group_small.throughput(criterion::Throughput::Bytes(1));
$(
    group_small.bench_function($id, |b| {
        b.iter_custom(|iters| {
                        //println!("=== new iteration, size {iters}");
            let (create, write_chunk, read_chunk) = help_with_type_inference($create, $write_chunk, $read_chunk);
            // Queue is very short in order to force a lot of contention between threads.
            let (mut p, mut c) = create(4);
            let push_thread = {
                std::thread::spawn(move || {
                    // The timing starts once both threads are ready.
                    let start = std::time::Instant::now();
                    let mut i = 0;
                    while i < iters {
                        //println!("write {i}");
                        // NB: we change chunk sizes in hope for unpredictable "skip" behavior.
        // TODO: more random. spin on 2, then optional 1?
                        while !write_chunk(&mut p, black_box(&[i as u8])) {
                            std::hint::spin_loop();
                        }
                        i += 1;
                        while !write_chunk(&mut p, black_box(&[i as u8, (i + 1) as u8])) {
                            std::hint::spin_loop();
                        }
                        i += 2;
                        //println!("write done {i}");
                    }
                    start
                })
            };
            // While the second thread is still starting up, this thread will busy-wait.
            let mut i = 0;
            while i < iters {
                        //println!("read {i}");
                let mut a = [0, 0];
                if read_chunk(&mut c, black_box(&mut a)) {
                    assert_eq!(a, [i as u8, (i + 1) as u8]);
                        //println!("read {i} success 2");
                    i += 2;
                }
                let mut a = [0];
                if read_chunk(&mut c, black_box(&mut a)) {
                    assert_eq!(a, [i as u8]);
                        //println!("read {i} success 1");
                    i += 1;
                }
            }
            // The timing stops once all items have been received.
            let stop = std::time::Instant::now();
            let start = push_thread.join().unwrap();
            stop.duration_since(start)
        });
    });
)+
    group_small.finish();
}

criterion_group!(benches, criterion_benchmark);
criterion_main!(benches);

    };
}

create_bip_benchmark! {
    "bip",
    rtrb::bip_arc::RingBuffer::new,
    |p, s| p.write_chunk(s.len()).map(|mut chunk| {
        chunk.as_mut_slice().copy_from_slice(s);
        chunk.commit_all();
    }).is_ok(),
    |c, s| c.read_chunk(s.len()).map(|chunk| {
        s.copy_from_slice(chunk.as_slice());
        chunk.commit_all();
    }).is_ok(),
    ::
    "bip2",
    rtrb::bip_arc2::RingBuffer::new,
    |p, s| p.write_chunk(s.len()).map(|mut chunk| {
        chunk.as_mut_slice().copy_from_slice(s);
        chunk.commit_all();
    }).is_ok(),
    |c, s| c.read_chunk(s.len()).map(|chunk| {
        s.copy_from_slice(chunk.as_slice());
        chunk.commit_all();
    }).is_ok(),
    ::
    "bip-uninit",
    rtrb::bip_arc::RingBuffer::new,
    |p, s| p.write_chunk_uninit(s.len()).map(|mut chunk| {
        use rtrb::CopyToUninit as _;
        s.copy_to_uninit(chunk.as_mut_slice());
        // SAFETY: All slots have been initialized.
        unsafe { chunk.commit_all() };
    }).is_ok(),
    |c, s| c.read_chunk(s.len()).map(|chunk| {
        s.copy_from_slice(chunk.as_slice());
        chunk.commit_all();
    }).is_ok(),
    ::
    "bip-push-pop",
    rtrb::bip_arc::RingBuffer::new,
    |p, s| {
        if p.slots() < s.len() {
            return false;
        }
        for x in s.iter() {
            p.push(*x).unwrap();
        }
        true
    },
    |c, s| {
        if c.slots() < s.len() {
            return false;
        }
        for x in s.iter_mut() {
            *x = c.pop().unwrap();
        }
        true
    },
    ::
    "mop-push-pop",
    rtrb::RingBuffer::new,
    |p, s| {
        if p.slots() < s.len() {
            return false;
        }
        for x in s.iter() {
            p.push(*x).unwrap();
        }
        true
    },
    |c, s| {
        if c.slots() < s.len() {
            return false;
        }
        for x in s.iter_mut() {
            *x = c.pop().unwrap();
        }
        true
    },
    ::
}
