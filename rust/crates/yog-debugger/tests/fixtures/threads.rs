// Compiled at test time via `rustc` — several threads running the same
// function while the main thread keeps raising SIGUSR1 at itself (handled),
// the way a JVM constantly fields its own signals. Line numbers here are
// load-bearing: `let mut total = x;` inside `work()` must stay on line 17.

use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

static HANDLED: AtomicU64 = AtomicU64::new(0);

extern "C" fn on_usr1(_signal: i32) {
    HANDLED.fetch_add(1, Ordering::SeqCst);
}

#[inline(never)]
fn work(x: u64) -> u64 {
    let mut total = x;
    total += 1;
    total
}

unsafe extern "C" {
    fn signal(signum: i32, handler: extern "C" fn(i32)) -> usize;
    fn raise(signum: i32) -> i32;
}

const SIGUSR1: i32 = 10;

fn main() {
    unsafe { signal(SIGUSR1, on_usr1) };
    for t in 0..4u64 {
        std::thread::spawn(move || {
            loop {
                std::hint::black_box(work(t));
                std::thread::sleep(Duration::from_millis(2));
            }
        });
    }
    while HANDLED.load(Ordering::SeqCst) < u64::MAX {
        unsafe { raise(SIGUSR1) };
        std::thread::sleep(Duration::from_millis(2));
    }
}
