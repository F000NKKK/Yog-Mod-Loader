// Compiled at test time via `rustc --crate-type cdylib -g` (see
// tests/symbol_table.rs) into a throwaway native library with real DWARF
// debug info, so `yog-symbols` has something genuine to resolve against
// without dragging a whole nested Cargo project into this crate's tests.

#[no_mangle]
pub extern "C" fn fixture_add(a: i64, b: i64) -> i64 {
    let sum = a + b;
    sum
}

#[no_mangle]
pub extern "C" fn fixture_greet() -> i64 {
    let mut total: i64 = 0;
    for i in 0..4 {
        total += i;
    }
    total
}
