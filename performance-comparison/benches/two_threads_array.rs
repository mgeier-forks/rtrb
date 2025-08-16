#[path = "../../benches/two_threads_array.rs"]
#[macro_use]
mod two_threads_array;

use two_threads_array::SIZE;

use ringbuf::traits::*;

create_two_threads_array_benchmark!(
    "rtrb::array",
    || {
        use rtrb::array::RingBuffer;
        static RB: RingBuffer<u8, SIZE> = RingBuffer::<u8, SIZE>::new();
        (RB.producer().unwrap(), RB.consumer().unwrap())
    },
    |p, i| p.push(i).is_ok(),
    |c| c.pop().ok(),
    ::
    "rtrb::array2",
    || {
        use rtrb::array2::RingBuffer;
        static RB: RingBuffer<u8, SIZE> = RingBuffer::<u8, SIZE>::new();
        (RB.producer().unwrap(), RB.consumer().unwrap())
    },
    |p, i| p.push(i).is_ok(),
    |c| c.pop().ok(),
    ::
    "rtrb::embedded",
    || {
        use rtrb::embedded::RingBuffer;
        static RB: RingBuffer<u8, SIZE> = RingBuffer::new();
        (RB.producer().unwrap(), RB.consumer().unwrap())
    },
    |p, i| p.push(i).is_ok(),
    |c| c.pop().ok(),
    ::
    "rtrb::embedded2",
    || {
        use rtrb::embedded2::RingBuffer;
        static RB: RingBuffer<u8, SIZE> = RingBuffer::new();
        (RB.producer().unwrap(), RB.consumer().unwrap())
    },
    |p, i| p.push(i).is_ok(),
    |c| c.pop().ok(),
    ::
    "rtrb::arc",
    || rtrb::arc::RingBuffer::<u8>::new(SIZE),
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
    ::
);
