// Fixture for Just My Code stepping: a user closure called from inside
// `std` (`OnceLock::get_or_init` -> `Once::call_once_force`, prebuilt
// without frame pointers). Stepping off the closure's end must come back
// to `compute`, not stop in `std`'s `once.rs`. The test finds lines by the
// markers.

use std::hint::black_box;
use std::sync::OnceLock;
use std::time::Duration;

#[inline(never)]
fn compute(seed: u64) -> u64 {
    let cell = OnceLock::new();
    let v = cell.get_or_init(|| {
        let a = black_box(seed) * 2; // CLOSURE_BODY
        a + 1
    });
    let out = *v + 1; // AFTER_INIT
    black_box(out)
}

fn main() {
    let mut seed = 0;
    loop {
        seed += 1;
        black_box(compute(seed));
        std::thread::sleep(Duration::from_millis(20));
    }
}
