// Compiled at test time via `rustc --crate-type bin -g` (see
// tests/ptrace_debugger.rs) — a throwaway standalone process for the
// debugger to attach to and set a real breakpoint against.

use std::sync::atomic::{AtomicI64, Ordering};
use std::time::Duration;

static COUNTER: AtomicI64 = AtomicI64::new(0);

#[inline(never)]
fn work() -> i64 {
    let mut total: i64 = 0;
    for i in 0..5 {
        total += i;
    }
    total
}

fn main() {
    loop {
        let value = work();
        COUNTER.store(value, Ordering::SeqCst);
        std::thread::sleep(Duration::from_millis(20));
    }
}
