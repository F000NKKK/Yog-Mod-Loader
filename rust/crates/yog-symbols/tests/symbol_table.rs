use std::path::PathBuf;
use std::process::Command;

use yog_symbols::SymbolTable;

/// Compiles `tests/fixtures/fixture.rs` into a real cdylib with DWARF debug
/// info via `rustc` directly (no nested Cargo project — this is a throwaway
/// fixture, not a crate we ship), returning the path to the built library.
/// Each test passes its own `tag` so parallel tests never build into (and
/// race on) the same output file.
fn build_fixture(tag: &str) -> PathBuf {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let src = manifest_dir.join("tests/fixtures/fixture.rs");
    let out_dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR"));
    let out = out_dir.join(format!("libyog_symbols_fixture_{tag}.{}", cdylib_ext()));

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
    let fixture = build_fixture("round_trip");
    let table = SymbolTable::load(&fixture).expect("loading fixture symbol table");

    // `fixture_add`'s `let sum = a + b;` line (see tests/fixtures/fixture.rs).
    let addrs = table.resolve_breakpoint("fixture.rs", 8);
    assert!(
        !addrs.is_empty(),
        "expected at least one address for fixture.rs:8"
    );

    let location = table
        .resolve_addr(addrs[0])
        .expect("resolving the breakpoint address back to a location");
    assert_eq!(location.line, 8);
    assert!(
        location.file.ends_with("fixture.rs"),
        "unexpected file: {:?}",
        location.file
    );

    // Not inlined at -O0: the chain is just the function itself.
    let chain = table.resolve_inlined(addrs[0]);
    assert_eq!(chain.len(), 1, "{chain:#?}");
    assert_eq!(chain[0], location);
}

#[test]
fn a_line_starts_at_a_statement() {
    let fixture = build_fixture("statements");
    let table = SymbolTable::load(&fixture).expect("loading fixture symbol table");

    let addrs = table.resolve_breakpoint("fixture.rs", 8);
    assert!(
        addrs.iter().any(|&a| table.is_statement_start(a)),
        "no statement starts at fixture.rs:8 ({addrs:x?})"
    );
    // Not a row at all — the byte after the line's first instruction.
    assert!(!table.is_statement_start(addrs[0] + 1) || addrs.contains(&(addrs[0] + 1)));
}

#[test]
fn lists_exported_functions() {
    let fixture = build_fixture("functions");
    let table = SymbolTable::load(&fixture).expect("loading fixture symbol table");

    let names: Vec<&str> = table.functions().map(|f| f.name.as_str()).collect();
    assert!(
        names.iter().any(|n| n.contains("fixture_add")),
        "functions: {names:?}"
    );
    assert!(
        names.iter().any(|n| n.contains("fixture_greet")),
        "functions: {names:?}"
    );
}

/// What an IDE passes: the absolute path of the open file — while the
/// DWARF says `../…` relative to the build dir, because `yog build`
/// compiles from `.yog-build/` with `path = "../src/lib.rs"`.
#[test]
fn absolute_path_matches_a_dotdot_relative_compile() {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let out = PathBuf::from(env!("CARGO_TARGET_TMPDIR"))
        .join(format!("libyog_symbols_fixture_dotdot.{}", cdylib_ext()));
    let status = Command::new(std::env::var("RUSTC").unwrap_or_else(|_| "rustc".into()))
        .current_dir(manifest_dir.join("tests"))
        .args([
            "--crate-type=cdylib",
            "-C",
            "debuginfo=2",
            "-C",
            "strip=none",
            "-o",
        ])
        .arg(&out)
        .arg("../tests/fixtures/fixture.rs")
        .status()
        .expect("failed to invoke rustc for fixture build");
    assert!(status.success(), "rustc failed to build the fixture cdylib");
    let table = SymbolTable::load(&out).expect("loading fixture symbol table");

    let source = manifest_dir.join("tests/fixtures/fixture.rs");
    let addrs = table.resolve_breakpoint(source.to_str().unwrap(), 8);
    assert!(!addrs.is_empty(), "absolute path didn't match");
    assert!(
        table
            .resolve_breakpoint(manifest_dir.join("other/fixture.rs").to_str().unwrap(), 8)
            .is_empty(),
        "a different absolute path must not match"
    );
    assert_eq!(table.resolve_addr(addrs[0]).unwrap().file, source);
}

/// The toolchain's `rust-objcopy`, as `yog build release` splits with it.
#[cfg(not(target_os = "macos"))]
fn objcopy() -> PathBuf {
    let out = Command::new("rustc")
        .args(["--print", "sysroot"])
        .output()
        .unwrap();
    let rustlib = PathBuf::from(String::from_utf8_lossy(&out.stdout).trim()).join("lib/rustlib");
    let file = format!("rust-objcopy{}", std::env::consts::EXE_SUFFIX);
    std::fs::read_dir(&rustlib)
        .unwrap()
        .flatten()
        .map(|host| host.path().join("bin").join(&file))
        .find(|p| p.is_file())
        .expect("the toolchain's rust-objcopy")
}

/// A release's native, stripped, with its debug info split beside it
/// (`<native>.debug`): the same lines found as in the whole one. Not on
/// macOS: a Mach-O keeps no debug info of its own to split (it's in the
/// `.dSYM` beside it).
#[cfg(not(target_os = "macos"))]
#[test]
fn a_stripped_native_reads_its_split_debug_info() {
    let whole = build_fixture("split");
    let stripped =
        whole.with_file_name(format!("libyog_symbols_fixture_stripped.{}", cdylib_ext()));
    let debug = stripped.with_added_extension("debug");
    let objcopy = objcopy();
    let run = |args: &[&std::ffi::OsStr]| {
        assert!(
            Command::new(&objcopy)
                .args(args)
                .status()
                .unwrap()
                .success()
        )
    };
    run(&[
        "--only-keep-debug".as_ref(),
        whole.as_os_str(),
        debug.as_os_str(),
    ]);
    run(&[
        "--strip-debug".as_ref(),
        whole.as_os_str(),
        stripped.as_os_str(),
    ]);
    let expected = SymbolTable::load(&whole)
        .unwrap()
        .resolve_breakpoint("fixture.rs", 8);
    let table = SymbolTable::load(&stripped).expect("the split debug info found beside it");
    assert!(!expected.is_empty());
    assert_eq!(table.resolve_breakpoint("fixture.rs", 8), expected);
    assert_eq!(table.resolve_addr(expected[0]).unwrap().line, 8);
}
