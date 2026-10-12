// Fixture for interrupting a step: F10 over `wait_forever()` never comes
// back on its own, so only Break All can end it.

use std::time::Duration;

#[inline(never)]
fn wait_forever() {
    loop {
        std::thread::sleep(Duration::from_millis(10));
    }
}

fn main() {
    std::thread::sleep(Duration::from_millis(300));
    wait_forever(); // CALL
}
