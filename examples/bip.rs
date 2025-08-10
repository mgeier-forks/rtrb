use rtrb::bip_arc::RingBuffer;

fn main() {
    let (_p, _c) = RingBuffer::<i32>::new(7);
}
