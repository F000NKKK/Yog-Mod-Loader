#![cfg(target_os = "linux")]

use std::path::PathBuf;
use std::process::Command;

use yog_debugger::maps::find_module_base;
use yog_debugger::ptrace_debugger::{interrupt, thread_name, Debugger, StopReason};
use yog_symbols::SymbolTable;

fn build_fixture() -> PathBuf {
    build("fixture")
}

fn build(name: &str) -> PathBuf {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let src = manifest_dir.join(format!("tests/fixtures/{name}.rs"));
    let out = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join(format!("yog_debugger_{name}_bin"));

    let status = Command::new(std::env::var("RUSTC").unwrap_or_else(|_| "rustc".into()))
        .args([
            "--crate-type=bin",
            "--edition=2024",
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
    let child = Command::new(&exe)
        .spawn()
        .expect("spawning fixture process");
    let _child = KillOnDrop(child);
    let pid = _child.0.id() as i32;

    let symbols = SymbolTable::load(&exe).expect("loading fixture's own debug info");
    // `let mut total: i64 = 0;` inside `work()` — see tests/fixtures/fixture.rs.
    let offsets = symbols.resolve_breakpoint("fixture.rs", TOTAL_LINE);
    assert!(
        !offsets.is_empty(),
        "expected at least one address for fixture.rs:{TOTAL_LINE}"
    );

    let mut debugger = Debugger::attach(pid).expect("attaching to fixture process");
    let base = find_module_base(debugger.pid(), &exe)
        .expect("locating the fixture binary's own load base in its own process");

    let addr = base + offsets[0];
    debugger.set_breakpoint(addr).expect("arming breakpoint");

    let stop = debugger.continue_().expect("continuing after attach");
    assert_eq!(
        stop,
        StopReason::Breakpoint(addr),
        "expected to stop at our breakpoint, got {stop:?}"
    );

    let location = symbols
        .resolve_addr(addr - base)
        .expect("resolving the hit address back to source");
    assert_eq!(location.line, TOTAL_LINE);

    let backtrace = debugger.backtrace(16).expect("walking the backtrace");
    assert!(!backtrace.is_empty());
    assert_eq!(
        backtrace[0], addr,
        "innermost frame should be the address we're stopped at"
    );

    debugger
        .clear_breakpoint(addr)
        .expect("clearing breakpoint");
    debugger.detach().expect("detaching");
}

/// `let mut total: i64 = 0;` inside `work()` in tests/fixtures/fixture.rs.
const TOTAL_LINE: u32 = 24;

/// Kills the fixture when the test ends — including when an assertion
/// fails — so a failing run doesn't leave it running (and holding the test
/// harness's output pipe open).
struct KillOnDrop(std::process::Child);

impl Drop for KillOnDrop {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

/// `let mut total = x;` inside `work()` in tests/fixtures/threads.rs.
const THREADS_WORK_LINE: u32 = 17;

/// Breakpoints in code every worker thread runs: hits must come from
/// threads other than the leader, the main thread's own (handled) SIGUSR1
/// stream must pass through untouched, an `interrupt` must surface as
/// `Interrupted`, and the process must keep running after detach.
#[test]
fn traces_every_thread_and_forwards_signals() {
    let exe = build("threads");
    let child = Command::new(&exe)
        .spawn()
        .expect("spawning threads fixture");
    let mut child = KillOnDrop(child);
    let pid = child.0.id() as i32;
    // Let the workers start so attach has to pick up existing threads.
    std::thread::sleep(std::time::Duration::from_millis(100));

    let symbols = SymbolTable::load(&exe).expect("loading fixture debug info");
    let offsets = symbols.resolve_breakpoint("threads.rs", THREADS_WORK_LINE);
    assert!(
        !offsets.is_empty(),
        "no address for threads.rs:{THREADS_WORK_LINE}"
    );

    let mut debugger = Debugger::attach(pid).expect("attaching");
    assert!(debugger.threads().len() >= 5, "expected leader + 4 workers");
    let base = find_module_base(debugger.pid(), &exe).expect("module base");
    let addr = base + offsets[0];
    debugger.set_breakpoint(addr).expect("arming breakpoint");

    let mut hit_threads = std::collections::HashSet::new();
    for _ in 0..20 {
        let stop = debugger.continue_().expect("continuing");
        assert_eq!(stop, StopReason::Breakpoint(addr));
        assert_eq!(debugger.registers().unwrap().rip, addr);
        // Non-stop: only the thread that hit it is stopped.
        assert!(
            !debugger.is_stopped(debugger.pid()),
            "the main thread kept running"
        );
        hit_threads.insert(debugger.current_thread());
    }
    assert!(
        !hit_threads.contains(&debugger.pid()),
        "only workers run work()"
    );
    assert!(hit_threads.len() > 1, "expected hits on several threads");

    debugger
        .clear_breakpoint(addr)
        .expect("clearing breakpoint");
    let interrupter = std::thread::spawn(move || {
        std::thread::sleep(std::time::Duration::from_millis(200));
        interrupt(pid);
    });
    let stop = debugger.continue_().expect("running until interrupted");
    interrupter.join().unwrap();
    assert_eq!(stop, StopReason::Interrupted);

    // Break All with no filter: every thread stops and can be inspected.
    debugger.pause().expect("pausing");
    for tid in debugger.threads() {
        debugger
            .select_thread(tid)
            .expect("every thread is stopped");
        assert_ne!(debugger.registers().expect("registers").rip, 0);
    }
    let interrupter = std::thread::spawn(move || {
        std::thread::sleep(std::time::Duration::from_millis(200));
        interrupt(pid);
    });
    assert_eq!(
        debugger.continue_().expect("resuming"),
        StopReason::Interrupted
    );
    interrupter.join().unwrap();

    // Break All with a filter: only the workers stop, the leader runs on.
    debugger.set_pause_filter(|name| name.starts_with("worker-"));
    debugger.pause().expect("pausing workers");
    let leader = debugger.pid();
    assert!(!debugger.is_stopped(leader), "the leader was let go");
    let workers: Vec<_> = debugger
        .threads()
        .into_iter()
        .filter(|&tid| thread_name(leader, tid).starts_with("worker-"))
        .collect();
    assert_eq!(workers.len(), 4);
    assert!(workers.iter().all(|&tid| debugger.is_stopped(tid)));
    assert!(workers.contains(&debugger.current_thread()));
    let interrupter = std::thread::spawn(move || {
        std::thread::sleep(std::time::Duration::from_millis(200));
        interrupt(pid);
    });
    let stop = debugger.continue_().expect("resuming after pause");
    interrupter.join().unwrap();
    assert_eq!(stop, StopReason::Interrupted);

    debugger.detach().expect("detaching");
    std::thread::sleep(std::time::Duration::from_millis(200));
    assert!(
        child.0.try_wait().expect("polling fixture").is_none(),
        "fixture died after detach"
    );
}
