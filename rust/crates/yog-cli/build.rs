//! Embeds workspace dependency versions at compile time.

use std::path::Path;

fn main() {
    let dir = std::env::var("CARGO_MANIFEST_DIR").unwrap();
    let mut path = Path::new(&dir);
    let mut rkyv_ver = String::new();
    let mut ctor_ver = String::new();

    // Walk up at most 5 levels to find workspace root Cargo.toml
    for _ in 0..5 {
        let candidate = path.join("Cargo.toml");
        if candidate.exists() {
            if let Ok(content) = std::fs::read_to_string(&candidate) {
                for line in content.lines() {
                    let line = line.trim();
                    if line.starts_with("rkyv = ") && rkyv_ver.is_empty() {
                        if let Some(ver) = line.split('"').nth(1) {
                            rkyv_ver = ver.to_string();
                        }
                    }
                    if line.starts_with("ctor = ") && ctor_ver.is_empty() {
                        if let Some(ver) = line.split('"').nth(1) {
                            ctor_ver = ver.to_string();
                        }
                    }
                }
                // Found both or reached the right file
                if !rkyv_ver.is_empty() || !ctor_ver.is_empty() {
                    break;
                }
            }
        }
        path = match path.parent() {
            Some(p) => p,
            None => break,
        };
    }

    if rkyv_ver.is_empty() { rkyv_ver = "0.8".into(); }
    if ctor_ver.is_empty() { ctor_ver = "1".into(); }

    println!("cargo:rustc-env=RKYV_VERSION={rkyv_ver}");
    println!("cargo:rustc-env=CTOR_VERSION={ctor_ver}");
}
