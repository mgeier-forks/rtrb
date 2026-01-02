use rtrb::arc::{PopError, PushError, RingBuffer};

fn main() {
    let (mut producer, mut consumer) = RingBuffer::new(4);
    let thread = std::thread::spawn(move || {
        for i in 10..25 {
            std::thread::sleep(std::time::Duration::from_secs(1));
            match producer.push(i) {
                Ok(()) => {
                    println!("pushed {i}");
                }
                Err(PushError::Full(_)) => {
                    println!("{i} was skipped");
                }
            }
        }
    });
    for _ in 0..15 {
        match consumer.pop() {
            Ok(i) => {
                println!("popped {i}");
            }
            Err(PopError::Empty) => {
                println!("waiting to pop ...");
            }
        }
        std::thread::sleep(std::time::Duration::from_secs(2));
    }
    println!("giving up");
    thread.join().unwrap();
}
