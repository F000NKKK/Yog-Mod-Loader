use std::path::PathBuf;
use std::process::Command;

use yog_symbols::SymbolTable;

/// Compiles `tests/fixtures/fixture.rs` into a real cdylib with DWARF debug
/// info via `rustc` directly (no nested Cargo project — this is a throwaway
/// fixture, not a crate we ship), returning the path to the built library.
fn build_fixture() -> PathBuf {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let src = manifest_dir.join("tests/fixtures/fixture.rs");
    let out_dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR"));
    let out = out_dir.join(format!("libyog_symbols_fixture.{}", cdylib_ext()));

    let status = Command::new(std::env::var("RUSTC").unwrap_or_else(|_| "rustc".into()))
        .args([
            "--crate-type=cdylib",
            "-C",
            "debuginfo=2",
            "-C",
            "strip=none",
            "-o",
        ])
        .arg(&out)
        .arg(&src)
        .status()
        .expect("failed to invoke rustc for fixture build");
    assert!(status.success(), "rustc failed to build the fixture cdylib");
    out
}

fn cdylib_ext() -> &'static str {
    if cfg!(target_os = "windows") {
        "dll"
    } else if cfg!(target_os = "macos") {
        "dylib"
    } else {
        "so"
    }
}

#[test]
fn resolves_breakpoint_and_address_round_trip() {
    let fixture = build_fixture();
    let table = SymbolTable::load(&fixture).expect("loading fixture symbol table");

    // `fixture_add`'s `let sum = a + b;` line (see tests/fixtures/fixture.rs).
    let addrs = table.resolve_breakpoint("fixture.rs", 8);
    assert!(!addrs.is_empty(), "expected at least one address for fixture.rs:8");

    let location = table.resolve_addr(addrs[0]).expect("resolving the breakpoint address back to a location");
    assert_eq!(location.line, 8);
    assert!(location.file.ends_with("fixture.rs"), "unexpected file: {:?}", location.file);
}

#[test]
fn lists_exported_functions() {
    let fixture = build_fixture();
    let table = SymbolTable::load(&fixture).expect("loading fixture symbol table");

    let names: Vec<&str> = table.functions().map(|f| f.name.as_str()).collect();
    assert!(names.iter().any(|n| n.contains("fixture_add")), "functions: {names:?}");
    assert!(names.iter().any(|n| n.contains("fixture_greet")), "functions: {names:?}");
}
