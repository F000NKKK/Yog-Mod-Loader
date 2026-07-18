// Compiled at test time via `rustc` — a throwaway process for the debugger
// to attach to, set breakpoints against, and step through. Line numbers
// here are load-bearing: the tests reference them directly. Key lines:
//   21  `let mut total: i64 = 0;`  (plain breakpoint target)
//   22  `total += inner(3);`       (a call — step-over vs step-into)
//   23  `total += 5;`              (where step-over from 22 lands)
//   11-14 body of `inner`          (where step-into from 22 lands)
// Keep this layout stable when editing.

use std::sync::atomic::{AtomicI64, Ordering};
use std::time::Duration;

#[inline(never)]
fn inner(x: i64) -> i64 {
    let doubled = x * 2;
    let plus = doubled + 1;
    plus
}

static COUNTER: AtomicI64 = AtomicI64::new(0);

#[inline(never)]
fn work() -> i64 {
    let mut total: i64 = 0;
    total += inner(3);
    total += 5;
    total
}

fn main() {
    loop {
        let value = work();
        COUNTER.store(value, Ordering::SeqCst);
        std::thread::sleep(Duration::from_millis(20));
    }
}
