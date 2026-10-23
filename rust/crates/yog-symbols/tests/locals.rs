//! Locals/parameters: static scope lookup against a real rustc-built
//! binary, plus (Linux x86_64) an end-to-end read of live values from a
//! ptrace-stopped fixture process.

use std::path::{Path, PathBuf};
use std::process::Command;

use yog_symbols::{LocalVar, SymbolTable, VarKind};

const FIXTURE: &str = "locals";
const MARKER: &str = "// LOCALS_HERE";

fn fixture_src() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(format!("tests/fixtures/{FIXTURE}.rs"))
}

/// Compiles `tests/fixtures/<name>.rs` into a debug binary. Each test
/// builds into its own output path so tests running in parallel never
/// race on the same file.
fn build(tag: &str) -> PathBuf {
    build_with(tag, &["-C", "opt-level=0"])
}

fn build_with(tag: &str, extra: &[&str]) -> PathBuf {
    let out =
        PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join(format!("yog_symbols_{FIXTURE}_{tag}_bin"));
    let status = Command::new(std::env::var("RUSTC").unwrap_or_else(|_| "rustc".into()))
        .args([
            "--crate-type=bin",
            "--edition=2024",
            "-g",
            "-C",
            "strip=none",
        ])
        .args(extra)
        .arg("-o")
        .arg(&out)
        .arg(fixture_src())
        .status()
        .expect("failed to invoke rustc for fixture build");
    assert!(status.success(), "rustc failed to build the locals fixture");
    out
}

fn marker_line() -> u32 {
    line_of(MARKER)
}

fn line_of(marker: &str) -> u32 {
    let src = std::fs::read_to_string(fixture_src()).unwrap();
    let idx = src
        .lines()
        .position(|l| l.contains(marker) && !l.trim_start().starts_with("//"))
        .unwrap_or_else(|| panic!("{marker} line in fixture"));
    idx as u32 + 1
}

fn addrs_of(table: &SymbolTable, marker: &str) -> Vec<u64> {
    let line = line_of(marker);
    let addrs = table.resolve_breakpoint(&format!("{FIXTURE}.rs"), line);
    assert!(!addrs.is_empty(), "no address for {FIXTURE}.rs:{line}");
    addrs
}

/// `name:in_scope` pairs, for compact assertions.
fn scoped(vars: &[LocalVar]) -> Vec<String> {
    vars.iter()
        .map(|v| format!("{}:{}", v.name, v.in_scope))
        .collect()
}

fn names(vars: &[LocalVar]) -> Vec<&str> {
    vars.iter().map(|v| v.name.as_str()).collect()
}

#[test]
fn lists_locals_and_parameters_at_a_line() {
    let exe = build("static");
    let table = SymbolTable::load(&exe).unwrap();
    let line = marker_line();
    let addrs = table.resolve_breakpoint(&format!("{FIXTURE}.rs"), line);
    assert!(!addrs.is_empty(), "no address for {FIXTURE}.rs:{line}");

    let vars = table.locals(addrs[0]);
    assert_eq!(
        names(&vars),
        [
            "count",
            "label",
            "x",
            "neg",
            "flag",
            "ch",
            "ratio",
            "s",
            "v",
            "p",
            "shape",
            "circle",
            "none",
            "some",
            "arr",
            "slice",
            "tuple",
            "r",
            "cell",
            "refcell",
            "once",
            "empty_once",
            "once_cell",
            "rc",
            "rc2",
            "arc",
            "nn",
            "pinned",
            "tagged",
            "mu",
            "md",
            "holder",
            "idx",
            "inner",
        ],
        "{vars:#?}"
    );
    assert_eq!(vars[0].kind, VarKind::Parameter);
    assert_eq!(vars[1].kind, VarKind::Parameter);
    assert!(vars[2..].iter().all(|v| v.kind == VarKind::Local));

    let ty = |n: &str| {
        vars.iter()
            .find(|v| v.name == n)
            .map(|v| v.type_name.as_str())
            .unwrap()
    };
    assert_eq!(ty("count"), "u32");
    assert_eq!(ty("label"), "&str");
    assert_eq!(ty("x"), "i32");
    assert_eq!(ty("s"), "alloc::string::String");
    assert!(ty("v").starts_with("alloc::vec::Vec<u8"), "{}", ty("v"));
    assert_eq!(ty("p"), "locals::Point");
    assert_eq!(ty("shape"), "locals::Shape");
    assert_eq!(ty("none"), "core::option::Option<u32>");
    assert_eq!(ty("arr"), "[u16; 3]");
    assert_eq!(ty("slice"), "&[u16]");
    assert_eq!(ty("tuple"), "(u8, i16)");
    assert_eq!(ty("r"), "&i32");
    let src = std::fs::read_to_string(fixture_src()).unwrap();
    let x_line = src
        .lines()
        .position(|l| l.contains("let x: i32 = 42;"))
        .unwrap() as u32
        + 1;
    assert_eq!(vars[2].decl_line, Some(x_line));
    // `total`'s block is a sibling of `inner`'s, not below it: not listed.
    assert!(vars.iter().all(|v| v.in_scope), "{vars:#?}");
}

/// Stopped on `let lines = ...;` the variable isn't in scope yet (rustc
/// opens its lexical block after the initializer) but is still listed, as
/// not live; on the next line it's live.
#[test]
fn lists_variables_declared_but_not_yet_live() {
    let exe = build("pending");
    let table = SymbolTable::load(&exe).unwrap();

    for addr in addrs_of(&table, "// PENDING_HERE") {
        let vars = table.locals(addr);
        assert_eq!(
            scoped(&vars),
            [
                "len:true",
                "expanded:true",
                "row_w:true",
                "lines:false",
                "padded:false"
            ],
            "at {addr:#x}"
        );
        let blind = table.read_local(
            &vars[3],
            &yog_symbols::FrameRegs::new(yog_symbols::Arch::host(), addr, 0, 0),
            0,
            &NoMem,
        );
        assert_eq!(blind.value, "<not in scope>");
        assert_eq!(blind.type_name, "f32");
    }

    let live = addrs_of(&table, "// LIVE_HERE");
    let vars = table.locals(live[0]);
    assert_eq!(
        scoped(&vars),
        [
            "len:true",
            "expanded:true",
            "row_w:true",
            "lines:true",
            "padded:true"
        ]
    );
}

/// Inside an inlined call: depth 0 is the inlined function's own frame,
/// depth 1 its caller's (with the caller's not-yet-assigned `r` pending), and past the outermost there's nothing.
#[test]
fn locals_per_inlined_frame() {
    let exe = build("inline");
    let table = SymbolTable::load(&exe).unwrap();
    let addr = addrs_of(&table, "// INLINE_HERE")[0];

    let frames = table.resolve_inlined(addr);
    assert!(frames.len() >= 2, "{frames:#?}");
    assert!(
        frames[1]
            .function
            .as_deref()
            .unwrap_or("")
            .ends_with("caller")
    );

    assert_eq!(table.locals(addr), table.locals_in_frame(addr, 0));
    assert_eq!(
        scoped(&table.locals_in_frame(addr, 0)),
        ["a:true", "doubled:true"]
    );
    assert_eq!(
        scoped(&table.locals_in_frame(addr, 1)),
        ["b:true", "before:true", "r:false"]
    );
    assert!(table.locals_in_frame(addr, 2).is_empty());
    assert_eq!(table.locals_in_frame(addr, 1)[0].kind, VarKind::Parameter);
}

struct NoMem;

impl yog_symbols::TargetMemory for NoMem {
    fn read(&self, _: u64, _: &mut [u8]) -> bool {
        false
    }
}

#[test]
fn no_locals_outside_any_function() {
    let exe = build("outside");
    let table = SymbolTable::load(&exe).unwrap();
    assert!(table.locals(u64::MAX - 1).is_empty());
}

#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
mod live {
    use super::*;

    use std::fs::File;
    use std::os::unix::fs::FileExt;
    use std::os::unix::process::CommandExt;

    use nix::sys::ptrace;
    use nix::sys::signal::Signal;
    use nix::sys::wait::{WaitStatus, waitpid};
    use nix::unistd::Pid;
    use yog_symbols::{FrameRegs, TargetMemory, Value, ValueNode};

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

    /// Lowest mapping of `exe` in the process: its load bias (PIE, first
    /// PT_LOAD at vaddr 0).
    fn load_base(pid: Pid, exe: &Path) -> u64 {
        let exe = std::fs::canonicalize(exe).unwrap();
        let maps = std::fs::read_to_string(format!("/proc/{pid}/maps")).unwrap();
        maps.lines()
            .filter(|l| l.split_whitespace().nth(5).map(Path::new) == Some(exe.as_path()))
            .filter_map(|l| u64::from_str_radix(l.split('-').next()?, 16).ok())
            .min()
            .expect("fixture binary mapped in its own process")
    }

    fn find<'a>(values: &'a [Value], name: &str) -> &'a Value {
        values
            .iter()
            .find(|v| v.name == name)
            .unwrap_or_else(|| panic!("no `{name}` in {values:#?}"))
    }

    fn child<'a>(value: &'a Value, name: &str) -> &'a Value {
        find(&value.children, name)
    }

    /// The fixture, stopped under ptrace at the marker line.
    struct Stopped {
        table: SymbolTable,
        regs: FrameRegs,
        base: u64,
        mem: ProcMem,
        _traced: Traced,
    }

    /// Runs the fixture under ptrace up to the marker line. `None` if the
    /// line has no code (an optimized build may fold it away).
    fn stop(exe: &Path) -> Option<Stopped> {
        let table = SymbolTable::load(exe).unwrap();
        let offsets = table.resolve_breakpoint(&format!("{FIXTURE}.rs"), marker_line());
        if offsets.is_empty() {
            return None;
        }

        let mut cmd = Command::new(exe);
        // SAFETY: only async-signal-safe work (a single ptrace syscall)
        // between fork and exec.
        unsafe {
            cmd.pre_exec(|| ptrace::traceme().map_err(std::io::Error::from));
        }
        let traced = Traced(cmd.spawn().expect("spawning fixture"));
        let pid = Pid::from_raw(traced.0.id() as i32);

        // Stopped at exec (SIGTRAP) — the image and its mappings exist.
        match waitpid(pid, None).unwrap() {
            WaitStatus::Stopped(_, Signal::SIGTRAP) => {}
            other => panic!("unexpected first stop: {other:?}"),
        }
        let base = load_base(pid, exe);

        // int3 on every address of the marker line; whichever runs first wins.
        for &off in &offsets {
            let addr = base + off;
            let word = ptrace::read(pid, addr as ptrace::AddressType).unwrap() as u64;
            let patched = (word & !0xff) | 0xcc;
            ptrace::write(pid, addr as ptrace::AddressType, patched as i64).unwrap();
        }
        ptrace::cont(pid, None).unwrap();
        match waitpid(pid, None).unwrap() {
            WaitStatus::Stopped(_, Signal::SIGTRAP) => {}
            other => panic!("expected breakpoint stop, got {other:?}"),
        }

        let r = ptrace::getregs(pid).unwrap();
        let pc = r.rip - 1;
        assert!(
            offsets.contains(&(pc - base)),
            "stopped at unexpected {pc:#x}"
        );
        let regs = FrameRegs::x86_64([
            r.rax, r.rdx, r.rcx, r.rbx, r.rsi, r.rdi, r.rbp, r.rsp, r.r8, r.r9, r.r10, r.r11,
            r.r12, r.r13, r.r14, r.r15, pc,
        ]);
        let mem = ProcMem(File::open(format!("/proc/{pid}/mem")).unwrap());
        Some(Stopped {
            table,
            regs,
            base,
            mem,
            _traced: traced,
        })
    }

    /// What `read_locals` makes of the fixture stopped at the marker line,
    /// plus the same read with all memory unreadable. `None` if the line
    /// has no code (an optimized build may fold it away).
    fn stop_and_read(exe: &Path) -> Option<(Vec<Value>, Vec<Value>)> {
        let Stopped {
            table,
            regs,
            base,
            mem,
            _traced,
        } = stop(exe)?;
        let values = table.read_locals(&regs, base, &mem);

        struct NoMem;
        impl TargetMemory for NoMem {
            fn read(&self, _: u64, _: &mut [u8]) -> bool {
                false
            }
        }
        let blind = table.read_locals(&regs, base, &NoMem);
        Some((values, blind))
    }

    fn check_exact(values: &[Value]) {
        let v = |n: &str| find(values, n).value.clone();
        assert_eq!(v("count"), "5");
        assert_eq!(v("label"), "\"label!\"");
        assert_eq!(v("x"), "42");
        assert_eq!(v("neg"), "-7");
        assert_eq!(v("flag"), "true");
        assert_eq!(v("ch"), "'Z'");
        assert_eq!(v("ratio"), "1.5");
        assert_eq!(v("s"), "\"hi\"");
        assert_eq!(v("v"), "len=3 [1, 2, 3]");
        assert_eq!(child(find(values, "v"), "[2]").value, "3");
        assert_eq!(v("p"), "{x: 3, y: -4}");
        assert_eq!(child(find(values, "p"), "y").value, "-4");
        assert_eq!(v("shape"), "Rect({x: 3, y: -4}, {x: 5, y: 6})");
        assert_eq!(child(child(find(values, "shape"), "1"), "x").value, "5");
        assert_eq!(v("circle"), "Circle {radius: 2.0}");
        assert_eq!(v("none"), "None");
        assert_eq!(v("some"), "Some(9)");
        assert_eq!(v("arr"), "[10, 20, 30]");
        assert_eq!(v("slice"), "len=2 [20, 30]");
        assert_eq!(v("tuple"), "(7, -1)");
        assert!(v("r").ends_with(" {42}"), "{}", v("r"));
        assert_eq!(child(find(values, "r"), "*r").value, "42");
        assert_eq!(v("inner"), "100");

        // std wrappers collapse to what they hold.
        assert_eq!(v("cell"), "11");
        assert_eq!(v("refcell"), "len=2 [4, 5]");
        assert_eq!(find(values, "refcell").type_name, "RefCell<Vec<u8>>");
        assert_eq!(find(values, "s").type_name, "String");
        assert_eq!(v("once"), "77");
        assert_eq!(v("empty_once"), "<uninit>");
        assert_eq!(v("once_cell"), "8");
        assert_eq!(v("rc"), "\"shared\" (strong=2)");
        assert_eq!(v("arc"), "31 (strong=1)");
        assert!(v("nn").ends_with(" {42}"), "{}", v("nn"));
        assert!(v("pinned").ends_with(" {12}"), "{}", v("pinned"));
        assert_eq!(v("tagged"), "{id: 3}");
        assert_eq!(find(values, "tagged").children.len(), 1);
        assert_eq!(v("mu"), "64");
        assert_eq!(v("md"), "16");
    }

    #[test]
    fn reads_live_values_at_a_breakpoint() {
        let exe = build("live");
        let (values, blind) = stop_and_read(&exe).expect("marker line has code at -O0");
        check_exact(&values);
        // Bad memory must degrade, not panic.
        assert_eq!(find(&blind, "x").value, "<unavailable>");
        assert_eq!(blind.len(), values.len());
    }

    /// Same, with `rbp`-based frames (different `DW_AT_frame_base`).
    #[test]
    fn reads_live_values_with_frame_pointers() {
        let exe = build_with(
            "live_fp",
            &["-C", "opt-level=0", "-C", "force-frame-pointers=yes"],
        );
        let (values, _) = stop_and_read(&exe).expect("marker line has code at -O0");
        check_exact(&values);
    }

    fn eval(s: &Stopped, expr: &str) -> Result<ValueNode, String> {
        s.table
            .evaluate(expr, &s.regs, s.base, &s.mem, 0)
            .map_err(|e| e.to_string())
    }

    fn value_of(s: &Stopped, expr: &str) -> String {
        match eval(s, expr) {
            Ok(node) => node.value,
            Err(e) => panic!("`{expr}`: {e}"),
        }
    }

    #[test]
    fn evaluates_watch_expressions() {
        let exe = build("eval");
        let s = stop(&exe).expect("marker line has code at -O0");
        let v = |e: &str| value_of(&s, e);
        assert_eq!(v("x"), "42");
        assert_eq!(v(" p.y "), "-4");
        assert_eq!(eval(&s, "p.y").unwrap().name, "p.y");
        assert_eq!(eval(&s, "p.y").unwrap().type_name, "i32");
        assert_eq!(v("shape.1.x"), "5");
        assert_eq!(v("circle.radius"), "2.0");
        assert_eq!(v("v[2]"), "3");
        assert_eq!(v("arr[idx]"), "20");
        assert_eq!(v("arr[0x2]"), "30");
        assert_eq!(v("*r"), "42");
        assert_eq!(v("slice[0]"), "20");
        assert_eq!(v("tuple.1"), "-1");
        assert_eq!(v("refcell[1]"), "5");
        assert_eq!(v("*rc"), "\"shared\"");
        assert_eq!(v("*arc"), "31");
        assert_eq!(v("*pinned"), "12");
        assert_eq!(v("*nn"), "42");
        assert_eq!(v("holder.items[1].y"), "4");
        assert_eq!(v("holder.items.len"), "2");
        assert_eq!(v("holder.boxed.x"), "9");
        assert_eq!(v("(*holder.boxed).y"), "8");
        assert_eq!(v("holder.pair.1.x"), "5");
        assert_eq!(v("holder.pair.0"), "1");
        assert_eq!(v("s"), "\"hi\"");
        assert_eq!(v("7"), "7");
        assert_eq!(v("LIMITS[2]"), "9");
        assert_eq!(v("locals::LIMITS"), "[7, 8, 9]");
        assert_eq!(v("config::SCALE"), "-3");
        assert_eq!(v("SCALE"), "-3");

        let err = |e: &str| eval(&s, e).unwrap_err();
        assert!(err("v[3]").contains("out of bounds"), "{}", err("v[3]"));
        assert!(err("nope").contains("cannot find value `nope`"));
        assert!(err("p.z").contains("no field `z`"));
        assert!(err("*x").contains("cannot be dereferenced"));
        assert!(err("x[0]").contains("cannot index"));
        assert!(err("label[0]").contains("cannot index"));
        assert!(err("p +").contains("unexpected"));
        assert!(eval(&s, "shape.radius").is_err());
    }

    /// Every row of the lazily expanded Locals tree, three levels deep:
    /// its `expr` evaluates back to the same value, and top-level rows
    /// agree with the eagerly formatted view.
    #[test]
    fn expands_lazily_and_round_trips_row_expressions() {
        let exe = build("expand");
        let s = stop(&exe).expect("marker line has code at -O0");
        let nodes = s.table.local_nodes(&s.regs, s.base, &s.mem, 0);
        let eager = s.table.read_locals(&s.regs, s.base, &s.mem);
        assert_eq!(nodes.len(), eager.len());
        for (lazy, full) in nodes.iter().zip(&eager) {
            assert_eq!(lazy.name, full.name);
            assert_eq!(lazy.value, full.value, "{}", lazy.name);
            assert_eq!(lazy.type_name, full.type_name, "{}", lazy.name);
            assert_eq!(
                lazy.children.is_some(),
                !full.children.is_empty(),
                "{}",
                lazy.name
            );
        }

        let names = |n: &[ValueNode]| n.iter().map(|c| c.name.clone()).collect::<Vec<_>>();
        let node = |name: &str| nodes.iter().find(|n| n.name == name).unwrap().clone();
        let open = |n: &ValueNode| s.table.expand(n.children.as_ref().unwrap(), &s.mem);
        let holder = open(&node("holder"));
        assert_eq!(names(&holder), ["items", "boxed", "pair"]);
        let items = open(&holder[0]);
        assert_eq!(names(&items), ["[0]", "[1]"]);
        assert_eq!(items[1].expr, "holder.items[1]");
        assert_eq!(items[1].value, "{x: 3, y: 4}");
        let boxed = open(&holder[1]);
        assert_eq!(boxed[0].expr, "*holder.boxed");
        assert_eq!(open(&boxed[0])[1].expr, "(*holder.boxed).y");
        assert_eq!(names(&open(&node("shape"))), ["0", "1"]);
        assert!(node("x").children.is_none());
        assert!(node("s").children.is_none());

        let mut checked = 0;
        let mut queue: Vec<(ValueNode, usize)> = nodes.into_iter().map(|n| (n, 0)).collect();
        while let Some((n, depth)) = queue.pop() {
            if n.value.starts_with('<') {
                continue;
            }
            let again = eval(&s, &n.expr).unwrap_or_else(|e| panic!("`{}`: {e}", n.expr));
            assert_eq!(again.value, n.value, "`{}`", n.expr);
            checked += 1;
            if depth < 3
                && let Some(c) = &n.children
            {
                queue.extend(
                    s.table
                        .expand(c, &s.mem)
                        .into_iter()
                        .map(|c| (c, depth + 1)),
                );
            }
        }
        assert!(checked > 60, "{checked}");
    }

    /// Optimized code: location lists and `DW_AT_const_value`s. Only values
    /// that stay meaningful under reordering are checked (at -O2 the marker
    /// line's code runs after `s`/`v` are already dropped — gdb reads the
    /// same freed bytes there); everything else must merely not panic.
    #[test]
    fn optimized_build_reads_or_degrades() {
        let exe = build_with("live_opt", &["-C", "opt-level=2"]);
        let Some((values, _)) = stop_and_read(&exe) else {
            return;
        };
        let ok = |name: &str, want: &str| {
            if let Some(v) = values.iter().find(|v| v.name == name) {
                assert!(
                    v.value == want || v.value.starts_with('<'),
                    "{name} = {:?}",
                    v.value
                );
            }
        };
        ok("count", "5");
        ok("x", "42");
        ok("neg", "-7");
        ok("flag", "true");
        ok("ch", "'Z'");
        ok("ratio", "1.5");
    }
}
