//! CFI unwinding through code built without frame pointers.

#![cfg(all(target_os = "linux", target_arch = "x86_64"))]

use std::fs::File;
use std::os::unix::fs::FileExt;
use std::os::unix::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::Command;

use nix::sys::ptrace;
use nix::sys::signal::Signal;
use nix::sys::wait::{WaitStatus, waitpid};
use nix::unistd::Pid;
use yog_symbols::{FrameRegs, SymbolTable, TargetMemory};

fn build(tag: &str) -> PathBuf {
    let src = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/unwind.rs");
    let out =
        PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join(format!("yog_symbols_unwind_{tag}_bin"));
    let status = Command::new(std::env::var("RUSTC").unwrap_or_else(|_| "rustc".into()))
        .args([
            "--crate-type=bin",
            "--edition=2024",
            "-g",
            "-C",
            "strip=none",
        ])
        .args(["-C", "force-frame-pointers=no", "-C", "opt-level=1"])
        .arg("-o")
        .arg(&out)
        .arg(src)
        .status()
        .expect("failed to invoke rustc for fixture build");
    assert!(status.success(), "rustc failed to build the unwind fixture");
    out
}

struct ProcMem(File);

impl TargetMemory for ProcMem {
    fn read(&self, addr: u64, buf: &mut [u8]) -> bool {
        self.0.read_exact_at(buf, addr).is_ok()
    }
}

/// Kills and reaps the traced child however the test exits.
struct Traced(std::process::Child);

impl Drop for Traced {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

fn load_base(pid: Pid, exe: &Path) -> u64 {
    let exe = std::fs::canonicalize(exe).unwrap();
    let maps = std::fs::read_to_string(format!("/proc/{pid}/maps")).unwrap();
    maps.lines()
        .filter(|l| l.split_whitespace().nth(5).map(Path::new) == Some(exe.as_path()))
        .filter_map(|l| u64::from_str_radix(l.split('-').next()?, 16).ok())
        .min()
        .expect("fixture binary mapped in its own process")
}

#[test]
fn backtrace_walks_frames_without_frame_pointers() {
    let exe = build("nofp");
    let table = SymbolTable::load(&exe).unwrap();

    let mut cmd = Command::new(&exe);
    // SAFETY: only async-signal-safe work (a single ptrace syscall)
    // between fork and exec.
    unsafe {
        cmd.pre_exec(|| ptrace::traceme().map_err(std::io::Error::from));
    }
    let traced = Traced(cmd.spawn().expect("spawning fixture"));
    let pid = Pid::from_raw(traced.0.id() as i32);
    match waitpid(pid, None).unwrap() {
        WaitStatus::Stopped(_, Signal::SIGTRAP) => {}
        other => panic!("unexpected first stop: {other:?}"),
    }
    let base = load_base(pid, &exe);

    // Runs into the fixture's own `int3` in `c`.
    ptrace::cont(pid, None).unwrap();
    match waitpid(pid, None).unwrap() {
        WaitStatus::Stopped(_, Signal::SIGTRAP) => {}
        other => panic!("expected the int3 stop, got {other:?}"),
    }
    let r = ptrace::getregs(pid).unwrap();
    let regs = FrameRegs::x86_64([
        r.rax, r.rdx, r.rcx, r.rbx, r.rsi, r.rdi, r.rbp, r.rsp, r.r8, r.r9, r.r10, r.r11, r.r12,
        r.r13, r.r14, r.r15, r.rip,
    ]);
    let mem = ProcMem(File::open(format!("/proc/{pid}/mem")).unwrap());

    let frames = table.backtrace(&regs, base, &mem, 64);
    let names: Vec<String> = frames
        .iter()
        .filter_map(|f| table.resolve_addr(f.pc().wrapping_sub(base))?.function)
        .collect();
    let position = |name: &str| {
        names
            .iter()
            .position(|n| n.rsplit("::").next() == Some(name))
            .unwrap_or_else(|| panic!("no `{name}` in {names:#?}"))
    };
    let (c, b, a, main) = (
        position("c"),
        position("b"),
        position("a"),
        position("main"),
    );
    assert!(c < b && b < a && a < main, "{names:#?}");
    assert_eq!(c, 0, "{names:#?}");

    // Every outer frame resolves to the line of its call.
    let src = std::fs::read_to_string(
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/unwind.rs"),
    )
    .unwrap();
    let line_of = |needle: &str| src.lines().position(|l| l.contains(needle)).unwrap() as u32 + 1;
    let line = |i: usize| table.resolve_addr(frames[i].pc() - base).unwrap().line;
    assert_eq!(line(b), line_of("black_box(c("));
    assert_eq!(line(a), line_of("black_box(b("));
    assert_eq!(line(main), line_of("let r = a("));
}
