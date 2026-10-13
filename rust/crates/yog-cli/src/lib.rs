//! The `yog` CLI's internals: `yog.toml` parsing, the generated
//! `.yog-build/` workspace, `.yres` resources, `content/` blocks and items,
//! `.yui` screens and `#[yog_export]` crates.
//!
//! Not for external use — a library only so the `yog` binary, its tests and
//! its benches share the code. No API stability, not published separately.

pub mod cargo_toml;
pub mod content;
pub mod exports;
pub mod manifest;
pub mod yres;
pub mod yui;
pub mod yui_component;
pub mod yui_grid;
pub mod yui_vm;
