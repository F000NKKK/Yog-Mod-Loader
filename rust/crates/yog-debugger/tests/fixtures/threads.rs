// Compiled at test time via `rustc` — threads running one function while the
// main thread makes trouble it handles itself (SIGUSR1; a caught panic on
// Windows), as a JVM fields its own signals. Line numbers here are
// load-bearing: `let mut total = x;` inside `work()` must stay on line 17.

use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

static HANDLED: AtomicU64 = AtomicU64::new(0);

#[cfg(unix)] extern "C" fn on_usr1(_signal: i32) {
    HANDLED.fetch_add(1, Ordering::SeqCst);
}

#[inline(never)]
fn work(x: u64) -> u64 {
    let mut total = x;
    total += 1;
    total
}

#[cfg(unix)]
unsafe extern "C" {
    fn signal(signum: i32, handler: extern "C" fn(i32)) -> usize;
    fn raise(signum: i32) -> i32;
}

#[cfg(unix)]
const SIGUSR1: i32 = 10;

/// Trouble the program handles itself: a signal it has a handler for.
#[cfg(unix)]
fn trouble() {
    unsafe { raise(SIGUSR1) };
}

/// Trouble the program handles itself: an exception it catches.
#[cfg(windows)]
fn trouble() {
    let _ = std::panic::catch_unwind(|| panic!("handled"));
    HANDLED.fetch_add(1, Ordering::SeqCst);
}

fn main() {
    #[cfg(unix)]
    unsafe {
        signal(SIGUSR1, on_usr1)
    };
    #[cfg(windows)]
    std::panic::set_hook(Box::new(|_| {}));
    for t in 0..4u64 {
        let worker = move || loop {
            std::hint::black_box(work(t));
            std::thread::sleep(Duration::from_millis(2));
        };
        std::thread::Builder::new()
            .name(format!("worker-{t}"))
            .spawn(worker)
            .unwrap();
    }
    while HANDLED.load(Ordering::SeqCst) < u64::MAX {
        trouble();
        std::thread::sleep(Duration::from_millis(2));
    }
}
