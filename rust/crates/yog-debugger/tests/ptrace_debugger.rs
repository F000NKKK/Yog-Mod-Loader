#![cfg(target_os = "linux")]

use std::path::PathBuf;
use std::process::Command;

use yog_debugger::maps::find_module_base;
use yog_debugger::ptrace_debugger::{Debugger, StepKind, StopReason};
use yog_symbols::SymbolTable;

fn build_fixture() -> PathBuf {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let src = manifest_dir.join("tests/fixtures/fixture.rs");
    let out = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("yog_debugger_fixture_bin");

    let status = Command::new(std::env::var("RUSTC").unwrap_or_else(|_| "rustc".into()))
        .args(["--crate-type=bin", "-C", "debuginfo=2", "-C", "strip=none", "-o"])
        .arg(&out)
        .arg(&src)
        .status()
        .expect("failed to invoke rustc for fixture build");
    assert!(status.success(), "rustc failed to build the fixture binary");
    out
}

/// End-to-end: spawn the fixture, attach, resolve a source breakpoint
/// through `yog-symbols`, arm it, continue, confirm the trap fires at the
/// expected address, and that `resolve_addr`/`backtrace` report sane
/// results from there.
#[test]
fn attaches_and_hits_a_real_breakpoint() {
    let exe = build_fixture();
    let mut child = Command::new(&exe).spawn().expect("spawning fixture process");
    let pid = child.id() as i32;

    let symbols = SymbolTable::load(&exe).expect("loading fixture's own debug info");
    // `let mut total: i64 = 0;` inside `work()` — see tests/fixtures/fixture.rs.
    let offsets = symbols.resolve_breakpoint("fixture.rs", 11);
    assert!(!offsets.is_empty(), "expected at least one address for fixture.rs:11");

    let mut debugger = Debugger::attach(pid).expect("attaching to fixture process");
    let base = find_module_base(debugger.pid(), &exe).expect("locating the fixture binary's own load base in its own process");

    let addr = base + offsets[0];
    debugger.set_breakpoint(addr).expect("arming breakpoint");

    let stop = debugger.continue_().expect("continuing after attach");
    assert_eq!(stop, StopReason::Breakpoint(addr), "expected to stop at our breakpoint, got {stop:?}");

    let location = symbols.resolve_addr(addr - base).expect("resolving the hit address back to source");
    assert_eq!(location.line, 11);

    let backtrace = debugger.backtrace(16).expect("walking the backtrace");
    assert!(!backtrace.is_empty());
    assert_eq!(backtrace[0], addr, "innermost frame should be the address we're stopped at");

    debugger.clear_breakpoint(addr).expect("clearing breakpoint");
    debugger.detach().expect("detaching");

    let _ = child.kill();
    let _ = child.wait();
}
