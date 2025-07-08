#[path = "../../benches/two_threads_array.rs"]
#[macro_use]
mod two_threads_array;

use two_threads_array::SIZE;

use ringbuf::traits::*;

create_two_threads_array_benchmark!(
    "rtrb",
    || {
        static RB: rtrb::array::RingBuffer<u8, SIZE> = rtrb::array::RingBuffer::<u8, SIZE>::new();
        (RB.producer().unwrap(), RB.consumer().unwrap())
    },
    |p, i| p.push(i).is_ok(),
    |c| c.pop().ok(),
    ::
    "rtrb2",
    || {
        static RB: rtrb::StaticRingBuffer2<u8, SIZE> = rtrb::StaticRingBuffer2::new();
        (RB.producer().unwrap(), RB.consumer().unwrap())
    },
    |p, i| p.push(i).is_ok(),
    |c| c.pop().ok(),
    ::
    "rtrb-embedded",
    || {
        static RB: rtrb::embedded::RingBuffer<u8, SIZE> = rtrb::embedded::RingBuffer::new();
        (RB.producer().unwrap(), RB.consumer().unwrap())
    },
    |p, i| p.push(i).is_ok(),
    |c| c.pop().ok(),
    ::
    "rtrb-dynamic",
    || rtrb::RingBuffer::<u8>::new(SIZE),
    |p, i| p.push(i).is_ok(),
    |c| c.pop().ok(),
    ::
    "ringbuffer-spsc",
    ringbuffer_spsc::RingBuffer::<u8, SIZE>::init,
    |p, i| p.push(i).is_none(),
    |c| c.pull(),
    ::
    "ringbuf",
    || Box::leak(Box::new(ringbuf::StaticRb::<u8, SIZE>::default())).split_ref(),
    |p, i| p.try_push(i).is_ok(),
    |c| c.try_pop(),
    ::
    "heapless",
    || Box::leak(Box::new(heapless::spsc::Queue::<u8, SIZE>::new())).split(),
    |p, i| p.enqueue(i).is_ok(),
    |c| c.dequeue(),
);
