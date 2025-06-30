#[path = "../../benches/two_threads_static.rs"]
#[macro_use]
mod two_threads;

use two_threads::SIZE;

use ringbuf::traits::*;

create_two_threads_static_benchmark!(

    "rtrb",
    || Box::leak(Box::new(rtrb::StaticRingBuffer::<u8, SIZE>::new())).split(),
    |p, i| p.push(i).is_ok(),
    |c| c.pop().ok();

    "rtrb2",
    || Box::leak(Box::new(rtrb::StaticRingBuffer2::<u8, SIZE>::new())).split(),
    |p, i| p.push(i).is_ok(),
    |c| c.pop().ok();

    "rtrb-embedded",
    || Box::leak(Box::new(rtrb::EmbeddedRingBuffer::<u8, SIZE>::new())).split(),
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
