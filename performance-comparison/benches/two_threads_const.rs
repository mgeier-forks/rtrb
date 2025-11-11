#[path = "../../benches/two_threads_const.rs"]
#[macro_use]
mod two_threads_const;

use ringbuf::traits::*;

create_two_threads_const_benchmark!(
    "rtrb",
    { ($N:expr) => {
        rtrb::RingBuffer::new($N)
    }},
    |p, i| p.push(i).is_ok(),
    |c| c.pop().ok(),
    ::
    "ringbuffer-spsc",
    { ($N:expr) => {
        ringbuffer_spsc::RingBuffer::<u8, $N>::init()
    }},
    |p, i| p.push(i).is_none(),
    |c| c.pull(),
    ::
    "ringbuf",
    { ($N:expr) => {
        Box::leak(Box::new(ringbuf::StaticRb::<u8, $N>::default())).split_ref()
    }},
    |p, i| p.try_push(i).is_ok(),
    |c| c.try_pop(),
    ::
    "heapless",
    { ($N:expr) => {
        Box::leak(Box::new(heapless::spsc::Queue::<u8, $N>::new())).split()
    }},
    |p, i| p.enqueue(i).is_ok(),
    |c| c.dequeue(),
    ::
    "ach-spsc",
    { ($N:expr) => {{
        let rb = Box::leak(Box::new(ach_spsc::Spsc::<u8, $N>::new()));
        (rb.take_sender().unwrap(), rb.take_recver().unwrap())
    }}},
    |p, i| p.try_send(i).is_ok(),
    |c| c.try_recv(),
    ::
);
