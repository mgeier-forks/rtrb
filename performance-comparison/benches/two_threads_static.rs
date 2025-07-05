#[path = "../../benches/two_threads_static.rs"]
#[macro_use]
mod two_threads;

use two_threads::SIZE;

use ringbuf::traits::*;

create_two_threads_static_benchmark!(

    "rtrb",
    || {
        static RB: rtrb::StaticRingBuffer<u8, SIZE> = rtrb::StaticRingBuffer::<u8, SIZE>::new();
        (RB.producer().unwrap(), RB.consumer().unwrap())
    },
    |p, i| p.push(i).is_ok(),
    |c| c.pop().ok();

    "rtrb2",
    || {
        static RB: rtrb::StaticRingBuffer2<u8, SIZE> = rtrb::StaticRingBuffer2::new();
        (RB.producer().unwrap(), RB.consumer().unwrap())
    },
    |p, i| p.push(i).is_ok(),
    |c| c.pop().ok();

    "rtrb-embedded",
    || {
        static RB: rtrb::EmbeddedRingBuffer<u8, SIZE> = rtrb::EmbeddedRingBuffer::new();
        (RB.producer().unwrap(), RB.consumer().unwrap())
    },
    |p, i| p.push(i).is_ok(),
    |c| c.pop().ok();

    "rtrb-dynamic",
    || rtrb::RingBuffer::<u8>::new(SIZE),
    |p, i| p.push(i).is_ok(),
    |c| c.pop().ok();

    "ringbuffer-spsc",
    ringbuffer_spsc::RingBuffer::<u8, SIZE>::init,
    |p, i| p.push(i).is_none(),
    |c| c.pull();

    "ringbuf",
    || Box::leak(Box::new(ringbuf::StaticRb::<u8, SIZE>::default())).split_ref(),
    |p, i| p.try_push(i).is_ok(),
    |c| c.try_pop();

    "heapless",
    || Box::leak(Box::new(heapless::spsc::Queue::<u8, SIZE>::new())).split(),
    |p, i| p.enqueue(i).is_ok(),
    |c| c.dequeue()

);
