#![cfg(target_os = "linux")]

use std::path::PathBuf;
use std::process::Command;
use std::time::{Duration, Instant};

use nix::unistd::Pid;
use yog_debugger::discovery::find_descendant_with_module;

fn build_fixture() -> PathBuf {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let src = manifest_dir.join("tests/fixtures/fixture.rs");
    let out = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("libyog_debugger_discovery_fixture.so");

    let status = Command::new(std::env::var("RUSTC").unwrap_or_else(|_| "rustc".into()))
        .args(["--crate-type=cdylib", "-o"])
        .arg(&out)
        .arg(&src)
        .status()
        .expect("failed to invoke rustc for fixture build");
    assert!(status.success(), "rustc failed to build the fixture cdylib");
    out
}

/// Finds the real target through an intermediate shell "wrapper" process —
/// the same shape as `./gradlew runClient` forking its own JVM: the process
/// we spawn is not the one with the module loaded, a *child* of it is.
#[test]
fn finds_module_through_an_intermediate_wrapper() {
    let fixture = build_fixture();

    // `LD_PRELOAD` maps the fixture .so into `sleep`'s address space, but
    // only for that one exec'd command — `sh` itself never has it mapped,
    // so this genuinely exercises "search past the wrapper," not "the
    // wrapper turned out to be the target after all."
    let mut wrapper = Command::new("sh")
        .arg("-c")
        .arg(format!("LD_PRELOAD={} sleep 30 & wait", fixture.display()))
        .spawn()
        .expect("spawning wrapper shell");

    // Give the shell a moment to actually fork+exec its child before we
    // start walking /proc for it.
    let deadline = Instant::now() + Duration::from_secs(5);
    let sleep_pid: i32 = loop {
        let out = Command::new("pgrep").args(["-P", &wrapper.id().to_string()]).output().expect("pgrep");
        let text = String::from_utf8_lossy(&out.stdout);
        if let Some(pid) = text.lines().next().and_then(|l| l.trim().parse().ok()) {
            break pid;
        }
        assert!(Instant::now() < deadline, "wrapper's child never appeared");
        std::thread::sleep(Duration::from_millis(20));
    };

    let found = find_descendant_with_module(Pid::from_raw(wrapper.id() as i32), "fixture");

    let _ = wrapper.kill();
    let _ = Command::new("kill").arg(sleep_pid.to_string()).status();
    let _ = wrapper.wait();

    assert_eq!(found, Some(Pid::from_raw(sleep_pid)), "should find the wrapper's child, not the wrapper itself");
    assert_ne!(found.unwrap().as_raw(), wrapper.id() as i32);
}
