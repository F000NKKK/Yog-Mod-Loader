//! Hot paths of a debugger stop, against the `tests/fixtures/locals.rs`
//! binary built with `rustc` at bench time (as the tests do): address and
//! line lookups, scope lookup, and — on Linux x86_64 — formatting, lazy
//! expansion and watch evaluation of the live locals of the fixture
//! stopped under ptrace. Target memory is cached page by page on first
//! read, so the measurements are the pure DWARF/formatting work, not
//! syscalls.

use std::hint::black_box;
use std::path::PathBuf;
use std::process::Command;

use criterion::{Criterion, criterion_group, criterion_main};
use yog_symbols::SymbolTable;

const FIXTURE: &str = "locals";
const MARKER: &str = "// LOCALS_HERE";

fn fixture_src() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(format!("tests/fixtures/{FIXTURE}.rs"))
}

fn build() -> PathBuf {
    let out =
        PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join(format!("yog_symbols_{FIXTURE}_bench_bin"));
    let status = Command::new(std::env::var("RUSTC").unwrap_or_else(|_| "rustc".into()))
        .args([
            "--crate-type=bin",
            "--edition=2024",
            "-g",
            "-C",
            "strip=none",
            "-C",
            "opt-level=0",
        ])
        .arg("-o")
        .arg(&out)
        .arg(fixture_src())
        .status()
        .expect("failed to invoke rustc for fixture build");
    assert!(status.success(), "rustc failed to build the locals fixture");
    out
}

fn marker_line() -> u32 {
    let src = std::fs::read_to_string(fixture_src()).unwrap();
    let idx = src
        .lines()
        .position(|l| l.contains(MARKER) && !l.trim_start().starts_with("//"))
        .expect("marker line in fixture");
    idx as u32 + 1
}

fn static_lookups(c: &mut Criterion) {
    let exe = build();
    let file = format!("{FIXTURE}.rs");
    let line = marker_line();

    c.bench_function("symbols/load", |b| {
        b.iter(|| SymbolTable::load(black_box(&exe)).unwrap())
    });

    let table = SymbolTable::load(&exe).unwrap();
    let addrs = table.resolve_breakpoint(&file, line);
    let pc = addrs[0];
    // Build the lazily initialized indexes outside the measurements.
    table.locals(pc);
    table.is_statement_start(pc);

    c.bench_function("symbols/resolve_breakpoint", |b| {
        b.iter(|| table.resolve_breakpoint(black_box(&file), black_box(line)))
    });
    c.bench_function("symbols/resolve_addr", |b| {
        b.iter(|| table.resolve_addr(black_box(pc)))
    });
    c.bench_function("symbols/resolve_inlined", |b| {
        b.iter(|| table.resolve_inlined(black_box(pc)))
    });
    c.bench_function("symbols/locals", |b| b.iter(|| table.locals(black_box(pc))));
}

#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
mod live {
    use std::cell::RefCell;
    use std::collections::HashMap;
    use std::fs::File;
    use std::os::unix::fs::FileExt;
    use std::os::unix::process::CommandExt;
    use std::path::Path;

    use nix::sys::ptrace;
    use nix::sys::signal::Signal;
    use nix::sys::wait::{WaitStatus, waitpid};
    use nix::unistd::Pid;
    use yog_symbols::{FrameRegs, TargetMemory};

    use super::*;

    const PAGE: u64 = 4096;

    /// `/proc/<pid>/mem`, each page read once and then served from memory.
    struct CachedMem {
        file: File,
        pages: RefCell<HashMap<u64, Option<Box<[u8]>>>>,
    }

    impl CachedMem {
        fn page(&self, index: u64) -> Option<Box<[u8]>> {
            self.pages
                .borrow_mut()
                .entry(index)
                .or_insert_with(|| {
                    let mut buf = vec![0u8; PAGE as usize];
                    self.file
                        .read_exact_at(&mut buf, index * PAGE)
                        .ok()
                        .map(|_| buf.into_boxed_slice())
                })
                .clone()
        }
    }

    impl TargetMemory for CachedMem {
        fn read(&self, addr: u64, buf: &mut [u8]) -> bool {
            let mut done = 0;
            while done < buf.len() {
                let at = addr.wrapping_add(done as u64);
                let Some(page) = self.page(at / PAGE) else {
                    return false;
                };
                let start = (at % PAGE) as usize;
                let n = (buf.len() - done).min(PAGE as usize - start);
                buf[done..done + n].copy_from_slice(&page[start..start + n]);
                done += n;
            }
            true
        }
    }

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

    pub fn stopped(c: &mut Criterion) {
        let exe = build();
        let table = SymbolTable::load(&exe).unwrap();
        let offsets = table.resolve_breakpoint(&format!("{FIXTURE}.rs"), marker_line());

        let mut cmd = Command::new(&exe);
        // SAFETY: only async-signal-safe work (a single ptrace syscall)
        // between fork and exec.
        unsafe {
            cmd.pre_exec(|| ptrace::traceme().map_err(std::io::Error::from));
        }
        let traced = Traced(cmd.spawn().expect("spawning fixture"));
        let pid = Pid::from_raw(traced.0.id() as i32);
        assert!(matches!(
            waitpid(pid, None).unwrap(),
            WaitStatus::Stopped(_, Signal::SIGTRAP)
        ));
        let base = load_base(pid, &exe);
        for &off in &offsets {
            let addr = (base + off) as ptrace::AddressType;
            let word = ptrace::read(pid, addr).unwrap() as u64;
            ptrace::write(pid, addr, ((word & !0xff) | 0xcc) as i64).unwrap();
        }
        ptrace::cont(pid, None).unwrap();
        assert!(matches!(
            waitpid(pid, None).unwrap(),
            WaitStatus::Stopped(_, Signal::SIGTRAP)
        ));
        let r = ptrace::getregs(pid).unwrap();
        let regs = FrameRegs::from_dwarf_order([
            r.rax,
            r.rdx,
            r.rcx,
            r.rbx,
            r.rsi,
            r.rdi,
            r.rbp,
            r.rsp,
            r.r8,
            r.r9,
            r.r10,
            r.r11,
            r.r12,
            r.r13,
            r.r14,
            r.r15,
            r.rip - 1,
        ]);
        let mem = CachedMem {
            file: File::open(format!("/proc/{pid}/mem")).unwrap(),
            pages: RefCell::default(),
        };

        // Warm the page cache and the table's lazy indexes.
        let nodes = table.local_nodes(&regs, base, &mem, 0);
        table.read_locals(&regs, base, &mem);
        let holder = nodes
            .iter()
            .find(|n| n.name == "holder")
            .and_then(|n| n.children.clone())
            .expect("expandable `holder`");
        const WATCH: &str = "holder.items[1].y";
        table.evaluate(WATCH, &regs, base, &mem, 0).unwrap();
        table.evaluate("SCALE", &regs, base, &mem, 0).unwrap();

        c.bench_function("symbols/read_locals", |b| {
            b.iter(|| table.read_locals(black_box(&regs), base, &mem))
        });
        c.bench_function("symbols/local_nodes", |b| {
            b.iter(|| table.local_nodes(black_box(&regs), base, &mem, 0))
        });
        c.bench_function("symbols/expand", |b| {
            b.iter(|| table.expand(black_box(&holder), &mem))
        });
        c.bench_function("symbols/evaluate_local", |b| {
            b.iter(|| table.evaluate(black_box(WATCH), &regs, base, &mem, 0))
        });
        c.bench_function("symbols/evaluate_static", |b| {
            b.iter(|| table.evaluate(black_box("SCALE"), &regs, base, &mem, 0))
        });
        drop(traced);
    }
}

#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
criterion_group!(benches, static_lookups, live::stopped);
#[cfg(not(all(target_os = "linux", target_arch = "x86_64")))]
criterion_group!(benches, static_lookups);
criterion_main!(benches);
