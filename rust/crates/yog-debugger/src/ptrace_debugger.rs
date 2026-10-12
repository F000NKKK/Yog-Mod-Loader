//! Real native breakpoint debugging via Linux `ptrace` — classic INT3
//! (`0xCC`) byte-patch breakpoints, `waitpid`-driven stop handling, and a
//! frame-pointer backtrace. Address resolution against source is delegated
//! entirely to `yog-symbols`; this module only knows about raw addresses in
//! the attached process.

use std::collections::{HashMap, HashSet};

use iced_x86::{Decoder, DecoderOptions, Mnemonic};
use nix::sys::ptrace;
use nix::sys::signal::Signal;
use nix::sys::wait::{waitpid, WaitPidFlag, WaitStatus};
use nix::unistd::Pid;
use yog_symbols::{SourceLocation, SymbolTable};

/// Which flavour of source-level step to perform — modelled on Visual
/// Studio's F10/F11/Shift+F11.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StepKind {
    /// F11 — descend into called user code (skips back out of library code
    /// with no source line, so you don't get lost inside `std`/formatting).
    Into,
    /// F10 — advance to the next source line in the current function,
    /// running any calls to completion rather than descending into them.
    Over,
    /// Shift+F11 — run until the current function returns to its caller.
    Out,
}

#[derive(Debug)]
pub enum DebugError {
    Ptrace(nix::Error),
    /// The tracee stopped for a reason other than one we asked for.
    UnexpectedStop(String),
}

impl std::fmt::Display for DebugError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DebugError::Ptrace(e) => write!(f, "ptrace: {e}"),
            DebugError::UnexpectedStop(s) => write!(f, "unexpected stop: {s}"),
        }
    }
}

impl std::error::Error for DebugError {}

/// Whether a stop means the tracee is gone (exited or killed) — a stepping
/// loop must bail immediately rather than trying to read its registers.
fn is_terminal(reason: &StopReason) -> bool {
    matches!(reason, StopReason::Exited(_) | StopReason::Killed(_))
}

impl From<nix::Error> for DebugError {
    fn from(e: nix::Error) -> Self {
        DebugError::Ptrace(e)
    }
}

/// Why the tracee stopped running, after a `cont`/`single_step`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StopReason {
    /// Hit one of our own breakpoints — already rewound/re-armed, `rip` is
    /// at the breakpoint address. Every thread is stopped; the one that hit
    /// it is [`Debugger::current_thread`].
    Breakpoint(u64),
    /// A single-step (or a step that ended between breakpoints) completed.
    Signal(Signal),
    /// [`interrupt`] reached the tracee: only the thread-group leader is
    /// stopped — enough to set/clear breakpoints — and the next `continue_`
    /// resumes it. Registers/backtraces aren't available until a real stop.
    Interrupted,
    Exited(i32),
    Killed(Signal),
}

/// Asks a traced process's tracer to stop and return
/// [`StopReason::Interrupted`] from its blocking `continue_`/`single_step`.
/// Callable from any thread — unlike every `ptrace` request, which only the
/// attaching thread may issue — so a UI thread can get breakpoints edited
/// while the tracee runs.
pub fn interrupt(pid: i32) {
    // `tgkill`, not `kill`: a process-directed SIGSTOP could land on any
    // thread and group-stop the whole JVM. The tracer swallows it (see
    // `Debugger::run_until_stop`), so it never reaches the tracee.
    // SAFETY: a plain syscall taking only integer arguments.
    unsafe {
        nix::libc::syscall(nix::libc::SYS_tgkill, pid, pid, nix::libc::SIGSTOP);
    }
}

/// Per-thread bookkeeping — a JVM has dozens of threads, any of which can
/// run into a breakpoint in mod code.
#[derive(Debug, Default)]
struct Thread {
    /// In a ptrace-stop (so registers/memory are accessible and it needs a
    /// `cont` to run again), as opposed to running.
    stopped: bool,
    /// A signal the thread received while we were stopping it, to deliver
    /// when it's resumed instead of dropping it.
    pending_signal: Option<Signal>,
}

/// One live `ptrace` session against a running process: every thread is
/// traced (threads created later are picked up automatically), signals the
/// tracee uses internally — the JVM relies on SIGSEGV for safepoints and
/// implicit null checks — are passed straight back to it, and a breakpoint
/// hit on any thread stops all of them.
pub struct Debugger {
    pid: Pid,
    threads: HashMap<Pid, Thread>,
    /// The stopped thread registers, backtraces and steps refer to — the
    /// one that last hit a breakpoint (the leader right after attach).
    current: Pid,
    /// Runtime address -> the original byte our `0xCC` replaced.
    breakpoints: HashMap<u64, u8>,
    /// Every address ever armed. A thread can trap on a breakpoint that's
    /// cleared before the trap is reported; its SIGTRAP must still be
    /// recognised as ours rather than delivered (which would kill the JVM).
    ever_armed: HashSet<u64>,
    /// Set right after a breakpoint hit: the address whose original byte is
    /// currently (temporarily) restored so the tracee sits paused exactly
    /// at the breakpoint for inspection. The next `continue_`/`single_step`
    /// must step over it and re-arm the `0xCC` before doing anything else,
    /// or the same breakpoint would never fire again.
    pending_rearm: Option<u64>,
}

/// `__WALL` so clone()d threads are reported; `__WNOTHREAD` so a
/// `waitpid(-1)` only reaps this thread's own children/tracees and never
/// steals exit statuses of processes spawned by other host threads.
fn wait_flags() -> Option<WaitPidFlag> {
    Some(WaitPidFlag::__WALL | WaitPidFlag::__WNOTHREAD)
}

fn task_ids(pid: Pid) -> Vec<Pid> {
    let Ok(dir) = std::fs::read_dir(format!("/proc/{pid}/task")) else {
        return Vec::new();
    };
    dir.filter_map(|e| e.ok()?.file_name().to_str()?.parse().ok())
        .map(Pid::from_raw)
        .collect()
}

impl Debugger {
    /// Attaches to every thread of `pid` and leaves them all stopped.
    pub fn attach(pid: i32) -> Result<Self, DebugError> {
        let pid = Pid::from_raw(pid);
        let mut debugger = Debugger {
            pid,
            threads: HashMap::new(),
            current: pid,
            breakpoints: HashMap::new(),
            ever_armed: HashSet::new(),
            pending_rearm: None,
        };
        ptrace::seize(pid, ptrace::Options::PTRACE_O_TRACECLONE)?;
        debugger.threads.insert(pid, Thread::default());
        // Threads can start while we enumerate; ones spawned by an already-
        // seized thread are auto-attached (TRACECLONE), the rest show up on
        // the next pass.
        loop {
            let fresh: Vec<Pid> = task_ids(pid)
                .into_iter()
                .filter(|tid| !debugger.threads.contains_key(tid))
                .collect();
            if fresh.is_empty() {
                break;
            }
            for tid in fresh {
                if ptrace::seize(tid, ptrace::Options::PTRACE_O_TRACECLONE).is_ok() {
                    debugger.threads.insert(tid, Thread::default());
                }
            }
        }
        debugger.stop_all()?;
        Ok(debugger)
    }

    pub fn pid(&self) -> Pid {
        self.pid
    }

    /// The thread registers, backtraces and steps currently refer to.
    pub fn current_thread(&self) -> Pid {
        self.current
    }

    /// Every traced thread id, leader first.
    pub fn threads(&self) -> Vec<Pid> {
        let mut tids: Vec<Pid> = self.threads.keys().copied().collect();
        tids.sort_by_key(|t| (*t != self.pid, t.as_raw()));
        tids
    }

    /// Makes `tid` the thread registers/backtraces/steps refer to. Only
    /// valid while it's stopped (after a breakpoint, everything is).
    pub fn select_thread(&mut self, tid: Pid) -> Result<(), DebugError> {
        match self.threads.get(&tid) {
            Some(t) if t.stopped => {
                self.current = tid;
                Ok(())
            }
            _ => Err(DebugError::UnexpectedStop(format!(
                "thread {tid} isn't stopped"
            ))),
        }
    }

    pub fn detach(&mut self) -> Result<(), DebugError> {
        // Only stopped tracees can be detached (or have memory patched).
        self.stop_all()?;
        for addr in self.breakpoints.keys().copied().collect::<Vec<_>>() {
            let _ = self.clear_breakpoint(addr);
        }
        for (tid, thread) in self.threads.drain() {
            let _ = ptrace::detach(tid, thread.pending_signal);
        }
        Ok(())
    }

    /// A stopped thread to read/write memory through (the address space is
    /// shared, but only a tracee in a ptrace-stop accepts the request).
    fn mem_thread(&self) -> Pid {
        if self.threads.get(&self.current).is_some_and(|t| t.stopped) {
            return self.current;
        }
        self.threads
            .iter()
            .find(|(_, t)| t.stopped)
            .map_or(self.current, |(&tid, _)| tid)
    }

    fn peek(&self, addr: u64) -> Result<i64, DebugError> {
        Ok(ptrace::read(
            self.mem_thread(),
            addr as ptrace::AddressType,
        )?)
    }

    fn poke(&self, addr: u64, data: i64) -> Result<(), DebugError> {
        ptrace::write(self.mem_thread(), addr as ptrace::AddressType, data)?;
        Ok(())
    }

    /// Arms an INT3 breakpoint at a real runtime address (already
    /// translated via [`crate::maps::find_module_base`] + a
    /// `yog-symbols`-resolved offset). No-op if already armed there.
    pub fn set_breakpoint(&mut self, addr: u64) -> Result<(), DebugError> {
        if self.breakpoints.contains_key(&addr) {
            return Ok(());
        }
        let word = self.peek(addr)?;
        let original_byte = (word & 0xff) as u8;
        self.breakpoints.insert(addr, original_byte);
        self.ever_armed.insert(addr);
        let patched = (word & !0xffi64) | 0xCC;
        self.poke(addr, patched)?;
        Ok(())
    }

    pub fn clear_breakpoint(&mut self, addr: u64) -> Result<(), DebugError> {
        if let Some(original_byte) = self.breakpoints.remove(&addr) {
            // If we're currently paused right at this breakpoint, its
            // original byte is already restored in memory — poking it
            // again is harmless, but there's no longer anything to step
            // over before the next continue/step.
            if self.pending_rearm == Some(addr) {
                self.pending_rearm = None;
            } else {
                let word = self.peek(addr)?;
                let restored = (word & !0xffi64) | original_byte as i64;
                self.poke(addr, restored)?;
            }
        }
        Ok(())
    }

    /// Turns an [`StopReason::Interrupted`] (only the leader stopped) into
    /// a full pause, like a debugger's Break All: every thread is brought to
    /// a stop so any of them can be inspected and stepped. The next
    /// [`continue_`](Self::continue_) resumes them all.
    pub fn pause(&mut self) -> Result<(), DebugError> {
        self.stop_all()?;
        if !self.threads.get(&self.current).is_some_and(|t| t.stopped) {
            self.current = self.pid;
        }
        Ok(())
    }

    /// Resumes every thread and runs until a breakpoint, an [`interrupt`],
    /// or the process ending.
    pub fn continue_(&mut self) -> Result<StopReason, DebugError> {
        if let Some(reason) = self.step_over_pending_breakpoint()? {
            return Ok(reason);
        }
        self.resume_all()?;
        self.run_until_stop()
    }

    /// Executes one machine instruction on the current thread; the others
    /// stay stopped. Right after a breakpoint hit, that instruction is the
    /// one the breakpoint sits on.
    pub fn single_step(&mut self) -> Result<StopReason, DebugError> {
        if self.pending_rearm.is_some() {
            return Ok(self
                .step_over_pending_breakpoint()?
                .unwrap_or(StopReason::Signal(Signal::SIGTRAP)));
        }
        self.step_current()
    }

    /// If we're paused right at a breakpoint (its original byte temporarily
    /// restored for inspection), execute that one real instruction and
    /// re-arm the `0xCC` before doing anything else the caller asked for —
    /// otherwise the same address could never trap again. `Some` only if
    /// the process ended during the step.
    fn step_over_pending_breakpoint(&mut self) -> Result<Option<StopReason>, DebugError> {
        let Some(addr) = self.pending_rearm.take() else {
            return Ok(None);
        };
        let reason = self.step_current()?;
        if matches!(reason, StopReason::Exited(_) | StopReason::Killed(_)) {
            return Ok(Some(reason));
        }
        if self.breakpoints.contains_key(&addr) {
            let word = self.peek(addr)?;
            self.poke(addr, (word & !0xffi64) | 0xCC)?;
        }
        Ok(None)
    }

    /// One instruction on `current`, absorbing stops that aren't the step
    /// itself: a signal arriving mid-step is delivered on the retried step
    /// (entering the handler, like gdb), a stale interrupt stop is retried.
    fn step_current(&mut self) -> Result<StopReason, DebugError> {
        let tid = self.current;
        let mut signal = self
            .threads
            .get_mut(&tid)
            .and_then(|t| t.pending_signal.take());
        loop {
            ptrace::step(tid, signal.take())?;
            self.mark(tid, false);
            match waitpid(tid, wait_flags())? {
                WaitStatus::Exited(_, code) => {
                    return Ok(self.thread_gone(tid, StopReason::Exited(code)))
                }
                WaitStatus::Signaled(_, sig, _) => {
                    return Ok(self.thread_gone(tid, StopReason::Killed(sig)));
                }
                WaitStatus::Stopped(_, Signal::SIGTRAP) => {
                    self.mark(tid, true);
                    return Ok(StopReason::Signal(Signal::SIGTRAP));
                }
                WaitStatus::Stopped(_, Signal::SIGSTOP) => {}
                WaitStatus::Stopped(_, sig) => signal = Some(sig),
                WaitStatus::PtraceEvent(_, _, event) if event == nix::libc::PTRACE_EVENT_CLONE => {
                    self.adopt_clone(tid)?;
                }
                _ => {}
            }
        }
    }

    /// A thread exiting isn't the process exiting unless it's the leader.
    fn thread_gone(&mut self, tid: Pid, reason: StopReason) -> StopReason {
        self.threads.remove(&tid);
        if tid == self.pid || self.threads.is_empty() {
            reason
        } else {
            // The stepping thread itself ended; fall back to the leader.
            self.current = self.pid;
            StopReason::Signal(Signal::SIGTRAP)
        }
    }

    fn mark(&mut self, tid: Pid, stopped: bool) {
        self.threads.entry(tid).or_default().stopped = stopped;
    }

    /// `tid` reported a clone event: track the new thread (it starts in a
    /// ptrace-stop of its own, which `run_until_stop` resumes).
    fn adopt_clone(&mut self, tid: Pid) -> Result<(), DebugError> {
        let child = Pid::from_raw(ptrace::getevent(tid)? as i32);
        self.threads.entry(child).or_default();
        Ok(())
    }

    fn resume_all(&mut self) -> Result<(), DebugError> {
        for (&tid, thread) in self.threads.iter_mut() {
            if thread.stopped {
                // A thread that vanished meanwhile reports its exit later.
                let _ = ptrace::cont(tid, thread.pending_signal.take());
                thread.stopped = false;
            }
        }
        Ok(())
    }

    /// Brings every running thread to a ptrace-stop. A thread that had
    /// already trapped on a breakpoint is rewound onto it, so it simply
    /// traps again (and gets reported) once resumed.
    fn stop_all(&mut self) -> Result<(), DebugError> {
        let running: Vec<Pid> = self
            .threads
            .iter()
            .filter(|(_, t)| !t.stopped)
            .map(|(&tid, _)| tid)
            .collect();
        for &tid in &running {
            let _ = ptrace::interrupt(tid);
        }
        let mut waiting = running;
        while let Some(tid) = waiting.pop() {
            let status = match waitpid(tid, wait_flags()) {
                Ok(s) => s,
                // Already reaped / not ours anymore.
                Err(_) => {
                    self.threads.remove(&tid);
                    continue;
                }
            };
            match status {
                WaitStatus::Exited(..) | WaitStatus::Signaled(..) => {
                    self.threads.remove(&tid);
                }
                WaitStatus::Stopped(_, Signal::SIGTRAP) => {
                    self.rewind_if_trapped(tid)?;
                    self.mark(tid, true);
                }
                // Our own `interrupt` — swallowed, never delivered.
                WaitStatus::Stopped(_, Signal::SIGSTOP) => self.mark(tid, true),
                WaitStatus::Stopped(_, sig) => {
                    let thread = self.threads.entry(tid).or_default();
                    thread.pending_signal = Some(sig);
                    thread.stopped = true;
                }
                WaitStatus::PtraceEvent(_, _, event) => {
                    if event == nix::libc::PTRACE_EVENT_CLONE {
                        let child = Pid::from_raw(ptrace::getevent(tid)? as i32);
                        if let std::collections::hash_map::Entry::Vacant(e) =
                            self.threads.entry(child)
                        {
                            e.insert(Thread::default());
                            waiting.push(child);
                        }
                    }
                    self.mark(tid, true);
                }
                _ => waiting.insert(0, tid),
            }
        }
        Ok(())
    }

    /// If `tid` just executed one of our INT3s, puts `rip` back on it.
    /// Returns the breakpoint address if so.
    fn rewind_if_trapped(&mut self, tid: Pid) -> Result<Option<u64>, DebugError> {
        let mut regs = ptrace::getregs(tid)?;
        let hit = regs.rip.wrapping_sub(1);
        if !self.ever_armed.contains(&hit) {
            return Ok(None);
        }
        regs.rip = hit;
        ptrace::setregs(tid, regs)?;
        Ok(Some(hit))
    }

    /// The run loop while every (known) thread is running: services
    /// thread creation and exit, hands every signal straight back to the
    /// tracee, and returns only for a breakpoint, an interrupt, or the end
    /// of the process.
    fn run_until_stop(&mut self) -> Result<StopReason, DebugError> {
        loop {
            let status = waitpid(None, wait_flags())?;
            let Some(tid) = status.pid() else { continue };
            match status {
                WaitStatus::Exited(_, code) => {
                    self.threads.remove(&tid);
                    if tid == self.pid {
                        return Ok(StopReason::Exited(code));
                    }
                }
                WaitStatus::Signaled(_, sig, _) => {
                    self.threads.remove(&tid);
                    if tid == self.pid {
                        return Ok(StopReason::Killed(sig));
                    }
                }
                WaitStatus::PtraceEvent(_, _, event) => {
                    if event == nix::libc::PTRACE_EVENT_CLONE {
                        self.adopt_clone(tid)?;
                    } else {
                        // A new thread's first stop, a stale interrupt, or
                        // a group-stop: just keep it running.
                        self.threads.entry(tid).or_default();
                    }
                    let _ = ptrace::cont(tid, None);
                }
                WaitStatus::Stopped(_, Signal::SIGTRAP) => {
                    self.mark(tid, true);
                    match self.rewind_if_trapped(tid)? {
                        Some(addr) if self.breakpoints.contains_key(&addr) => {
                            return self.report_breakpoint(tid, addr);
                        }
                        // A breakpoint cleared before this trap surfaced:
                        // rewound onto the original instruction, carry on.
                        Some(_) => {
                            self.mark(tid, false);
                            ptrace::cont(tid, None)?;
                        }
                        // Not ours — the tracee's own SIGTRAP.
                        None => {
                            self.mark(tid, false);
                            ptrace::cont(tid, Signal::SIGTRAP)?;
                        }
                    }
                }
                WaitStatus::Stopped(_, Signal::SIGSTOP) => {
                    self.mark(tid, true);
                    return Ok(StopReason::Interrupted);
                }
                WaitStatus::Stopped(_, sig) => {
                    let _ = ptrace::cont(tid, sig);
                }
                _ => {}
            }
        }
    }

    fn report_breakpoint(&mut self, tid: Pid, addr: u64) -> Result<StopReason, DebugError> {
        self.current = tid;
        self.stop_all()?;
        // Restore the real instruction byte, but do NOT step past it yet —
        // the thread stays genuinely paused at `addr` until the next
        // continue/step, so backtraces/inspection see the real location.
        let original_byte = self.breakpoints[&addr];
        let word = self.peek(addr)?;
        self.poke(addr, (word & !0xffi64) | original_byte as i64)?;
        self.pending_rearm = Some(addr);
        Ok(StopReason::Breakpoint(addr))
    }

    pub fn registers(&self) -> Result<nix::libc::user_regs_struct, DebugError> {
        Ok(ptrace::getregs(self.current)?)
    }

    /// The source location `rip` currently maps to, or `None` if the current
    /// instruction is in code with no line info (a library, or a
    /// compiler-generated stub). `base` is the mod native's load address —
    /// see `crate::maps::find_module_base_by_prefix`.
    pub fn source_location(&self, symbols: &SymbolTable, base: u64) -> Option<SourceLocation> {
        let rip = self.registers().ok()?.rip;
        let offset = rip.checked_sub(base)?;
        symbols.resolve_addr(offset)
    }

    /// A comparable (file, line) key for "are we still on the same source
    /// line" checks during stepping — line 0 (no line) collapses to `None`.
    fn line_key(&self, symbols: &SymbolTable, base: u64) -> Option<(std::path::PathBuf, u32)> {
        let loc = self.source_location(symbols, base)?;
        if loc.line == 0 {
            return None;
        }
        Some((loc.file, loc.line))
    }

    /// Reads `buf.len()` bytes of the tracee's memory at `addr`, substituting
    /// back the original instruction bytes for any breakpoints armed within
    /// the window — so a downstream `0xCC` never corrupts the decode of the
    /// instruction we're actually looking at.
    fn read_code(&self, addr: u64, buf: &mut [u8]) -> Result<(), DebugError> {
        let mut i = 0;
        while i < buf.len() {
            let word = self.peek(addr + i as u64)?;
            let bytes = word.to_ne_bytes();
            let take = (buf.len() - i).min(bytes.len());
            buf[i..i + take].copy_from_slice(&bytes[..take]);
            i += bytes.len();
        }
        for (&bp_addr, &orig) in &self.breakpoints {
            if bp_addr >= addr && bp_addr < addr + buf.len() as u64 {
                buf[(bp_addr - addr) as usize] = orig;
            }
        }
        Ok(())
    }

    /// Decodes the instruction at `rip`, returning `(is_call, length)`.
    /// Falls back to `(false, 1)` on unreadable/undecodable memory — a
    /// caller then just single-steps a byte's worth conservatively.
    fn decode_at(&self, rip: u64) -> (bool, u64) {
        let mut buf = [0u8; 16];
        if self.read_code(rip, &mut buf).is_err() {
            return (false, 1);
        }
        let mut decoder = Decoder::with_ip(64, &buf, rip, DecoderOptions::NONE);
        let insn = decoder.decode();
        if insn.is_invalid() {
            return (false, 1);
        }
        (matches!(insn.mnemonic(), Mnemonic::Call), insn.len() as u64)
    }

    /// Runs until `addr` is reached (via a temporary breakpoint) — used to
    /// step over a call by returning to the instruction after it. Stops
    /// early and reports it if a *user* breakpoint is hit along the way.
    /// The temporary breakpoint only counts for the stepping thread back in
    /// the stepping frame: other threads running the same code, or a
    /// recursive call reaching it deeper in the stack, just pass through.
    fn run_to(&mut self, addr: u64) -> Result<StopReason, DebugError> {
        let was_user_bp = self.breakpoints.contains_key(&addr);
        if !was_user_bp {
            self.set_breakpoint(addr)?;
        }
        let origin = self.current;
        let origin_sp = self.registers()?.rsp;
        let result = loop {
            let reason = match self.continue_() {
                Ok(r) => r,
                Err(e) => break Err(e),
            };
            match reason {
                // Breakpoint edits wait for the step to finish.
                StopReason::Interrupted => {}
                StopReason::Breakpoint(hit) if hit == addr && !was_user_bp => {
                    let here = self.registers().map(|r| r.rsp).unwrap_or(0);
                    if self.current == origin && here >= origin_sp {
                        break Ok(StopReason::Signal(Signal::SIGTRAP));
                    }
                }
                other => break Ok(other),
            }
        };
        if !was_user_bp {
            let _ = self.clear_breakpoint(addr);
        }
        result
    }

    /// A source-level step (F10/F11/Shift+F11) — see [`StepKind`]. Keeps
    /// executing single instructions (skipping over calls per `kind`) until
    /// the source line changes appropriately, a user breakpoint is hit, or
    /// the tracee exits. `symbols`/`base` resolve `rip` to a source line.
    pub fn source_step(
        &mut self,
        kind: StepKind,
        symbols: &SymbolTable,
        base: u64,
    ) -> Result<StopReason, DebugError> {
        let start = self.line_key(symbols, base);
        let start_sp = self.registers()?.rsp;
        // A generous ceiling so a pathological step (e.g. into a huge
        // library call whose return we somehow miss) can't spin forever —
        // reaching it just stops where we are, same as any other trap.
        const MAX_STEPS: usize = 5_000_000;

        for _ in 0..MAX_STEPS {
            let rip = self.registers()?.rip;
            let (is_call, len) = self.decode_at(rip);

            let reason = if is_call && kind == StepKind::Over {
                self.run_to(rip + len)?
            } else if is_call && kind == StepKind::Into {
                let stepped = self.single_step()?;
                if is_terminal(&stepped) {
                    return Ok(stepped);
                }
                // If we descended into code with no source line (a library
                // call), run straight back out to the instruction after the
                // call rather than crawling through it instruction by
                // instruction — matches VS's "step into stays in your code".
                if self.line_key(symbols, base).is_none() {
                    let back = self.run_to(rip + len)?;
                    if is_terminal(&back) || matches!(back, StopReason::Breakpoint(_)) {
                        return Ok(back);
                    }
                }
                StopReason::Signal(Signal::SIGTRAP)
            } else {
                self.single_step()?
            };

            match reason {
                StopReason::Exited(_) | StopReason::Killed(_) => return Ok(reason),
                StopReason::Breakpoint(_) => return Ok(reason),
                StopReason::Signal(Signal::SIGTRAP) | StopReason::Interrupted => {}
                StopReason::Signal(other) => return Ok(StopReason::Signal(other)),
            }

            let cur = self.line_key(symbols, base);
            let cur_sp = self.registers()?.rsp;
            let stop = match kind {
                // Stack grows down: a strictly *higher* rsp means we've
                // returned into a shallower frame.
                StepKind::Out => cur_sp > start_sp && cur.is_some(),
                StepKind::Into | StepKind::Over => cur.is_some() && cur != start,
            };
            if stop {
                return Ok(StopReason::Signal(Signal::SIGTRAP));
            }
        }
        Ok(StopReason::Signal(Signal::SIGTRAP))
    }

    /// Frame-pointer-walk backtrace: return addresses only, in innermost-
    /// first order, capped to avoid running away on a corrupt/omitted frame
    /// chain. Callers resolve each address through `yog-symbols` after
    /// subtracting the relevant module's load base.
    ///
    /// This is the simple, robust technique (same fallback gdb uses) — it
    /// relies on Rust's debug builds keeping `rbp` frame pointers. A CFI
    /// (`.eh_frame`) based unwinder would also work for frame-pointer-
    /// omitted release builds, and is a reasonable future upgrade, not
    /// needed for this pass's debug-build-only use case.
    pub fn backtrace(&self, max_frames: usize) -> Result<Vec<u64>, DebugError> {
        let regs = self.registers()?;
        let mut frames = vec![regs.rip];
        let mut bp = regs.rbp;

        while bp != 0 && frames.len() < max_frames {
            let Ok(saved_bp) = self.peek(bp) else { break };
            let Ok(return_addr) = self.peek(bp.wrapping_add(8)) else {
                break;
            };
            if return_addr == 0 {
                break;
            }
            frames.push(return_addr as u64);
            bp = saved_bp as u64;
        }

        Ok(frames)
    }
}
