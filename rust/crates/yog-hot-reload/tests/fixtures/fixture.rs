// Compiled at test time via `rustc --crate-type cdylib` (see
// tests/hot_reload.rs) — a throwaway native library standing in for a
// mod's compiled artifact.

#[no_mangle]
pub extern "C" fn fixture_version() -> i64 {
    1
}
