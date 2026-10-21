#![cfg(target_os = "linux")]

use std::path::PathBuf;
use std::process::Command;

use yog_debugger::maps::find_module_base;
use yog_debugger::ptrace_debugger::{Debugger, StopReason, interrupt, thread_name};
use yog_symbols::SymbolTable;

fn build_fixture() -> PathBuf {
    build("fixture")
}

fn build(name: &str) -> PathBuf {
    build_as(name, name)
}

/// Builds fixture `name` to its own output `tag` — tests run in parallel,
/// and one rebuilding a binary another is running breaks the latter's
/// module lookup (the mapping turns `(deleted)`).
fn build_as(name: &str, tag: &str) -> PathBuf {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let src = manifest_dir.join(format!("tests/fixtures/{name}.rs"));
    let out = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join(format!("yog_debugger_{tag}_bin"));

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

/// F10 walks `work()` line by line, and off its last line lands on the
/// caller's next statement rather than mid-line or lost in code without
/// lines.
#[test]
fn step_over_walks_lines_and_returns_to_the_caller() {
    use yog_debugger::StepKind;

    let exe = build_as("fixture", "fixture_steps");
    let child = Command::new(&exe)
        .spawn()
        .expect("spawning fixture process");
    let _child = KillOnDrop(child);
    let pid = _child.0.id() as i32;

    let symbols = SymbolTable::load(&exe).expect("loading fixture's own debug info");
    let mut debugger = Debugger::attach(pid).expect("attaching to fixture process");
    let base = find_module_base(debugger.pid(), &exe).expect("fixture load base");
    let addr = base + symbols.resolve_breakpoint("fixture.rs", 26)[0];
    debugger.set_breakpoint(addr).expect("arming breakpoint");
    assert_eq!(
        debugger.continue_().expect("continuing"),
        StopReason::Breakpoint(addr)
    );
    debugger
        .clear_breakpoint(addr)
        .expect("clearing breakpoint");

    let mut lines = Vec::new();
    for _ in 0..3 {
        debugger
            .source_step(StepKind::Over, &symbols, base)
            .expect("stepping over");
        let location = debugger
            .source_location(&symbols, base)
            .expect("stopped somewhere with lines");
        lines.push(location.line);
    }
    // 27 `total`, 28 the closing brace, then back in `main` on the
    // statement after the call.
    assert_eq!(lines, [27, 28, 33]);
    debugger.detach().expect("detaching");
}

/// Just My Code: F10 off the end of a closure that `std` called
/// (`OnceLock::get_or_init`) lands back in the user function that called
/// into `std` — never in `std`'s own sources on the way.
#[test]
fn step_over_leaves_std_for_the_user_caller() {
    use yog_debugger::{StepKind, is_library_source};

    let exe = build_as("jmc", "jmc_steps");
    let src = std::fs::read_to_string(
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/jmc.rs"),
    )
    .unwrap();
    let line_of = |marker: &str| {
        src.lines()
            .position(|l| l.contains(marker) && !l.trim_start().starts_with("//"))
            .unwrap() as u32
            + 1
    };
    let (init, body, after) = (
        line_of("get_or_init("),
        line_of("// CLOSURE_BODY"),
        line_of("// AFTER_INIT"),
    );

    let child = Command::new(&exe).spawn().expect("spawning jmc fixture");
    let _child = KillOnDrop(child);
    let pid = _child.0.id() as i32;

    let symbols = SymbolTable::load(&exe).expect("loading fixture debug info");
    let mut debugger = Debugger::attach(pid).expect("attaching");
    let base = find_module_base(debugger.pid(), &exe).expect("fixture load base");
    let addr = base + symbols.resolve_breakpoint("jmc.rs", body)[0];
    debugger.set_breakpoint(addr).expect("arming breakpoint");
    assert_eq!(
        debugger.continue_().expect("continuing"),
        StopReason::Breakpoint(addr)
    );
    debugger
        .clear_breakpoint(addr)
        .expect("clearing breakpoint");

    // The CFI backtrace reaches `compute` through `std`'s frames.
    let frames = debugger
        .backtrace_with(&symbols, base, 64)
        .expect("backtrace");
    let names: Vec<String> = frames
        .iter()
        .filter_map(|f| symbols.resolve_addr(f.pc() - base)?.function)
        .collect();
    assert!(names.iter().any(|n| n.ends_with("::compute")), "{names:#?}");

    let mut seen = Vec::new();
    for _ in 0..4 {
        let stop = debugger
            .source_step(StepKind::Over, &symbols, base)
            .expect("stepping over");
        assert_eq!(stop, StopReason::Signal(nix::sys::signal::Signal::SIGTRAP));
        let location = debugger
            .source_location(&symbols, base)
            .expect("stopped somewhere with lines");
        assert!(
            !is_library_source(&location.file),
            "stopped in library code: {location:?} after {seen:?}"
        );
        let in_compute = location
            .function
            .as_deref()
            .is_some_and(|f| f.ends_with("::compute"));
        seen.push((location.line, location.function));
        if in_compute {
            break;
        }
    }
    let (line, function) = seen.last().unwrap();
    assert!(
        function
            .as_deref()
            .is_some_and(|f| f.ends_with("::compute")),
        "never got back to `compute`: {seen:?}"
    );
    // Back on the call's line (its result is still being stored), or past it.
    assert!(*line == init || *line == after, "{seen:?}");
    debugger.detach().expect("detaching");
}

/// Break All reaches a step that runs over a call that never returns: with
/// the pause request raised, `source_step` gives up with `Interrupted`.
#[test]
fn pause_request_interrupts_a_step_over_a_blocking_call() {
    use yog_debugger::StepKind;
    use yog_sync::Arc;
    use yog_sync::atomic::{AtomicBool, Ordering};

    let exe = build_as("blocking", "blocking_pause");
    let src = std::fs::read_to_string(
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/blocking.rs"),
    )
    .unwrap();
    let call = src.lines().position(|l| l.contains("// CALL")).unwrap() as u32 + 1;

    let child = Command::new(&exe)
        .spawn()
        .expect("spawning blocking fixture");
    let _child = KillOnDrop(child);
    let pid = _child.0.id() as i32;

    let symbols = SymbolTable::load(&exe).expect("loading fixture debug info");
    let mut debugger = Debugger::attach(pid).expect("attaching");
    let base = find_module_base(debugger.pid(), &exe).expect("fixture load base");
    let addr = base + symbols.resolve_breakpoint("blocking.rs", call)[0];
    debugger.set_breakpoint(addr).expect("arming breakpoint");
    assert_eq!(
        debugger.continue_().expect("continuing"),
        StopReason::Breakpoint(addr)
    );

    let pause = Arc::new(AtomicBool::new(false));
    debugger.set_pause_request(pause.clone());

    // A breakpoint edit's interrupt (no pause request) doesn't end the step...
    let edit = std::thread::spawn(move || {
        std::thread::sleep(std::time::Duration::from_millis(200));
        interrupt(pid);
    });
    // ...Break All does.
    let flag = pause.clone();
    let breaker = std::thread::spawn(move || {
        std::thread::sleep(std::time::Duration::from_millis(600));
        flag.store(true, Ordering::SeqCst);
        interrupt(pid);
    });
    let started = std::time::Instant::now();
    let stop = debugger
        .source_step(StepKind::Over, &symbols, base)
        .expect("stepping over");
    edit.join().unwrap();
    breaker.join().unwrap();
    assert_eq!(stop, StopReason::Interrupted);
    assert!(started.elapsed() >= std::time::Duration::from_millis(500));

    pause.store(false, Ordering::SeqCst);
    debugger.pause().expect("pausing");
    assert_ne!(debugger.registers().expect("registers").rip, 0);
    debugger.detach().expect("detaching");
}

/// Attaches to a fresh `ticks` fixture with a breakpoint on `tick`'s first
/// line, stopped at its first hit.
fn ticks_session(tag: &str) -> (KillOnDrop, Debugger, SymbolTable, u64, u64) {
    let exe = build_as("ticks", tag);
    let src = std::fs::read_to_string(
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/ticks.rs"),
    )
    .unwrap();
    let first = src
        .lines()
        .position(|l| l.contains("// TICK_FIRST"))
        .unwrap() as u32
        + 1;

    let child = KillOnDrop(Command::new(&exe).spawn().expect("spawning ticks fixture"));
    let symbols = SymbolTable::load(&exe).expect("loading fixture debug info");
    let mut debugger = Debugger::attach(child.0.id() as i32).expect("attaching");
    let base = find_module_base(debugger.pid(), &exe).expect("fixture load base");
    let addr = base + symbols.resolve_breakpoint("ticks.rs", first)[0];
    debugger.set_breakpoint(addr).expect("arming breakpoint");
    assert_eq!(
        debugger.continue_().expect("continuing"),
        StopReason::Breakpoint(addr)
    );
    (child, debugger, symbols, base, addr)
}

/// Stepping off the end of a function nothing in user code called runs on
/// to the next hit — on a new thread, the stepped one having exited — and
/// the session keeps working from there, hit after hit.
#[test]
fn step_off_a_thread_entry_runs_on_to_the_next_hit() {
    use yog_debugger::StepKind;

    let (_child, mut debugger, symbols, base, addr) = ticks_session("ticks_step_off");
    for round in 0..3 {
        let first = debugger.current_thread();
        let mut stop = None;
        for _ in 0..10 {
            let reason = debugger
                .source_step(StepKind::Over, &symbols, base)
                .unwrap_or_else(|e| panic!("stepping (round {round}): {e}"));
            if reason != StopReason::Signal(nix::sys::signal::Signal::SIGTRAP) {
                stop = Some(reason);
                break;
            }
        }
        assert_eq!(stop, Some(StopReason::Breakpoint(addr)));
        assert_ne!(debugger.current_thread(), first, "a new tick thread hit it");
        assert_eq!(debugger.registers().expect("registers").rip, addr);
    }
    for _ in 0..3 {
        assert_eq!(
            debugger.continue_().expect("continuing"),
            StopReason::Breakpoint(addr)
        );
    }
    debugger.detach().expect("detaching");
}

/// The regression behind "debugging works only once": a step that turned
/// into a free run, interrupted by a breakpoint edit (no pause request),
/// left the host resuming the *step* — whose thread was long gone — and
/// every register read failed with `ESRCH`. Resuming must just run on.
#[test]
fn interrupted_free_run_step_resumes_cleanly() {
    use yog_debugger::StepKind;

    let (child, mut debugger, symbols, base, addr) = ticks_session("ticks_interrupt");
    let pid = child.0.id() as i32;
    debugger
        .clear_breakpoint(addr)
        .expect("clearing breakpoint");

    let stepped = debugger.current_thread();
    let edit = std::thread::spawn(move || {
        std::thread::sleep(std::time::Duration::from_millis(300));
        interrupt(pid);
    });
    // Shift+F11 out of `tick` lands in `std`, with no user caller: a free run.
    let stop = debugger
        .source_step(StepKind::Out, &symbols, base)
        .expect("stepping out");
    edit.join().unwrap();
    assert_eq!(stop, StopReason::Interrupted);
    assert!(
        !debugger.threads().contains(&stepped),
        "the stepped thread exited during the run"
    );

    // The edit itself: arm the breakpoint again while interrupted.
    debugger.set_breakpoint(addr).expect("re-arming breakpoint");
    let stop = debugger
        .source_step(StepKind::Over, &symbols, base)
        .expect("resuming the interrupted step");
    assert_eq!(stop, StopReason::Breakpoint(addr));
    assert_eq!(
        debugger.continue_().expect("continuing"),
        StopReason::Breakpoint(addr)
    );
    debugger.detach().expect("detaching");
}

/// A process killed while paused at a breakpoint ends the session as an
/// exit, not as a `ptrace` error.
#[test]
fn killed_while_paused_reports_the_exit() {
    let (mut child, mut debugger, _symbols, _base, _addr) = ticks_session("ticks_killed");
    child.0.kill().expect("killing the fixture");
    let stop = debugger.continue_().expect("continuing after the kill");
    assert_eq!(stop, StopReason::Killed(nix::sys::signal::Signal::SIGKILL));
}
