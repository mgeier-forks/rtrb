use rtrb::bip::RingBuffer;

fn main() {
    let (p, c) = RingBuffer::<i32>::new(7);
}
