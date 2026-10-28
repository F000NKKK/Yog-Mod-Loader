//! The substrate-data version this CLI is built on, for the mods' generated
//! `Cargo.toml` (`SUBSTRATE_DATA_VERSION`): read off this crate's manifest —
//! the workspace's line in the repository, the published crate's own
//! `[dependencies.substrate-data]` table on crates.io — so it has one source.

use std::path::Path;

const DEPENDENCY: &str = "substrate-data";
const WORKSPACE_MANIFEST: &str = "../../Cargo.toml";

fn main() {
    let dir = std::env::var("CARGO_MANIFEST_DIR").expect("cargo sets CARGO_MANIFEST_DIR");
    let own = Path::new(&dir).join("Cargo.toml");
    let workspace = Path::new(&dir).join(WORKSPACE_MANIFEST);
    println!("cargo::rerun-if-changed={}", own.display());
    println!("cargo::rerun-if-changed={}", workspace.display());
    let version = [own, workspace]
        .iter()
        .filter_map(|p| std::fs::read_to_string(p).ok())
        .find_map(|text| version_in(&text))
        .expect("the manifest names substrate-data's version");
    println!("cargo::rustc-env=SUBSTRATE_DATA_VERSION={version}");
}

/// The version `text` (a manifest) gives substrate-data: `substrate-data =
/// "x"`, or a `version = "x"` in its `[dependencies.substrate-data]` table.
fn version_in(text: &str) -> Option<String> {
    let table = format!("[dependencies.{DEPENDENCY}]");
    let mut in_table = false;
    for line in text.lines().map(str::trim) {
        if line.starts_with('[') {
            in_table = line == table;
            continue;
        }
        let key = if in_table { "version" } else { DEPENDENCY };
        let Some(rest) = line.strip_prefix(key).map(str::trim_start) else {
            continue;
        };
        let value = rest
            .strip_prefix('=')
            .map(str::trim)
            .and_then(|v| v.strip_prefix('"'));
        if let Some(value) = value {
            return value.split('"').next().map(str::to_string);
        }
    }
    None
}
