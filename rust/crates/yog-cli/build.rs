//! Embeds the rkyv version from the workspace Cargo.toml at compile time.

use std::path::Path;

fn main() {
    // Walk up from OUT_DIR to find the workspace root Cargo.toml
    let dir = std::env::var("CARGO_MANIFEST_DIR").unwrap();
    let mut path = Path::new(&dir);
    // Walk up at most 5 levels
    for _ in 0..5 {
        let candidate = path.join("Cargo.toml");
        if candidate.exists() {
            if let Ok(content) = std::fs::read_to_string(&candidate) {
                for line in content.lines() {
                    let line = line.trim();
                    if line.starts_with("rkyv = ") {
                        if let Some(ver) = line.split('"').nth(1) {
                            println!("cargo:rustc-env=RKYV_VERSION={ver}");
                            return;
                        }
                    }
                }
            }
        }
        path = match path.parent() {
            Some(p) => p,
            None => break,
        };
    }
    // Fallback
    println!("cargo:rustc-env=RKYV_VERSION=0.8");
}
