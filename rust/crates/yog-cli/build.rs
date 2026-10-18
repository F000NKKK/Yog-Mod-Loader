//! Embeds workspace dependency versions at compile time.
//!
//! In the workspace the crate's own `Cargo.toml` says `rkyv = { workspace = true }`
//! and the version is in the root's `[workspace.dependencies]`; a published
//! package has it inlined. So: the nearest manifest, upward, that names one.

use std::path::Path;

/// `dep = "x"` or `dep = { version = "x", ... }` → `x`.
fn version_of(manifest: &str, dep: &str) -> Option<String> {
    manifest.lines().find_map(|line| {
        let (name, value) = line.split_once('=')?;
        if name.trim() != dep {
            return None;
        }
        let value = value.trim();
        let quoted = match value.strip_prefix('{') {
            Some(table) => table.split_once("version")?.1.split_once('=')?.1,
            None => value,
        };
        Some(quoted.split('"').nth(1)?.to_string())
    })
}

fn main() {
    let dir = std::env::var("CARGO_MANIFEST_DIR").expect("cargo sets CARGO_MANIFEST_DIR");
    let rkyv = Path::new(&dir)
        .ancestors()
        .filter_map(|p| std::fs::read_to_string(p.join("Cargo.toml")).ok())
        .find_map(|m| version_of(&m, "rkyv"))
        .expect("rkyv's version: in this manifest or the workspace root's");
    println!("cargo:rustc-env=RKYV_VERSION={rkyv}");
}
