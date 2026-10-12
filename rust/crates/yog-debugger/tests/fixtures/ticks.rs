// Fixture for steps that end in a free run: `tick` runs on a fresh thread
// each time, the way a JVM calls into a mod — stepping off its end leaves
// user code for good (only `std` and libc are up that thread's stack), and
// the thread exits right after. The test finds lines by the markers.

use std::hint::black_box;
use std::time::Duration;

#[inline(never)]
fn tick() {
    let a = black_box(1u64); // TICK_FIRST
    let b = black_box(a + 1);
    black_box(b);
}

fn main() {
    loop {
        std::thread::spawn(tick).join().unwrap();
        std::thread::sleep(Duration::from_millis(20));
    }
}
