//! A mod's project: `yog.toml`, the generated `Cargo.toml` and Rust names,
//! its `#[yog_export]` crate, its benchmarks.

pub mod bench;
pub mod cargo_toml;
pub mod exports;
pub mod manifest;
pub(crate) mod rust_code;
