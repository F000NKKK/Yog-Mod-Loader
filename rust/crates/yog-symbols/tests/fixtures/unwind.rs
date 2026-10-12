// Fixture for the CFI unwinding test: built with
// `-C force-frame-pointers=no -C opt-level=1`, so only `.eh_frame` can walk
// the stack. Each call's result is used afterwards so none becomes a tail
// call.

use std::hint::black_box;

#[inline(never)]
fn c(x: u64) -> u64 {
    let y = black_box(x) * 3;
    // SAFETY: a breakpoint trap; the test runs this under ptrace.
    unsafe { std::arch::asm!("int3") };
    y + 1
}

#[inline(never)]
fn b(x: u64) -> u64 {
    black_box(c(black_box(x) + 1)) + 7
}

#[inline(never)]
fn a(x: u64) -> u64 {
    black_box(b(black_box(x) * 2)) + 11
}

fn main() {
    let r = a(black_box(5));
    println!("{}", black_box(r));
}
