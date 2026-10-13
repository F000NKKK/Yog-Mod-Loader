//! Real native breakpoint debugging via Linux `ptrace` — classic INT3
//! (`0xCC`) byte-patch breakpoints, `waitpid`-driven stop handling, and a
//! frame-pointer backtrace. Address resolution against source is delegated
//! entirely to `yog-symbols`; this module only knows about raw addresses in
//! the attached process.

use std::collections::{HashMap, HashSet};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use iced_x86::{Decoder, DecoderOptions, Mnemonic};
use nix::sys::ptrace;
use nix::sys::signal::Signal;
use nix::sys::wait::{waitpid, WaitPidFlag, WaitStatus};
use nix::unistd::Pid;
use yog_symbols::{FrameRegs, SourceLocation, SymbolTable, TargetMemory};

use crate::{is_library_source, user_location};

type LineKey = (std::path::PathBuf, u32);

/// The x86 `INT3` opcode a breakpoint patches in.
const INT3: u8 = 0xCC;

/// How far up the stack [`Debugger::leave_foreign_code`] looks for a user
/// caller to return to.
const CALLER_SEARCH_DEPTH: usize = 64;

/// `word` (a little-endian memory word at some address) with its first
/// byte — the one at that address — replaced by `byte`.
fn with_first_byte(word: i64, byte: u8) -> i64 {
    (word & !0xff) | i64::from(byte)
}

/// Where a step currently is.
enum Position {
    /// On a line of user code.
    User(LineKey),
    /// In user code, on a compiler-generated row without a line.
    NoLine,
    /// In library code (`std`, a dependency), or in code with no line info
    /// at all (outside the module, a stub).
    Foreign,
}

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

/// `ESRCH`: the thread a request named isn't in a ptrace-stop any more —
/// in practice it was killed (the process exiting) between reporting a
/// stop and the request. One such thread mustn't end the whole session:
/// its exit is still reported by `waitpid` later, like any other thread's.
fn vanished(e: &nix::Error) -> bool {
    *e == nix::errno::Errno::ESRCH
}

/// What became of single-stepping one thread.
enum Step {
    /// The step completed (`Signal(SIGTRAP)`), or the process ended.
    Stopped(StopReason),
    /// The thread exited (or was killed) instead of completing the step;
    /// it's no longer tracked, and `current` moved to another thread.
    Vanished,
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
/// hit stops only the thread that hit it — the rest of the process (its
/// sound, its server thread, ...) keeps running, like gdb's non-stop mode.
/// [`pause`](Debugger::pause) stops only the threads its filter selects.
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
    /// currently (temporarily) restored so the thread that hit it sits
    /// paused exactly at the breakpoint for inspection, and that thread.
    /// The next `continue_`/`single_step` must step *that thread* over it
    /// and re-arm the `0xCC` before doing anything else, or the same
    /// breakpoint would never fire again.
    pending_rearm: Option<(u64, Pid)>,
    /// Which threads (by name) [`pause`](Debugger::pause) stops; all when
    /// unset.
    pause_filter: Option<PauseFilter>,
    /// Set by the host (see [`Debugger::set_pause_request`]) when an
    /// [`interrupt`] means Break All rather than a breakpoint edit.
    pause_request: Option<Arc<AtomicBool>>,
    /// The last [`source_step`](Debugger::source_step) had turned into a
    /// free run when an [`interrupt`] broke into it: there's no step left
    /// to resume, only that run.
    step_ran_free: bool,
}

type PauseFilter = Box<dyn Fn(&str) -> bool + Send>;

/// `__WALL` so clone()d threads are reported; `__WNOTHREAD` so a
/// `waitpid(-1)` only reaps this thread's own children/tracees and never
/// steals exit statuses of processes spawned by other host threads.
fn wait_flags() -> Option<WaitPidFlag> {
    Some(WaitPidFlag::__WALL | WaitPidFlag::__WNOTHREAD)
}

/// A thread's name (`comm`, as `ps -L` shows it) — empty if it's gone.
pub fn thread_name(pid: Pid, tid: Pid) -> String {
    std::fs::read_to_string(format!("/proc/{pid}/task/{tid}/comm"))
        .map(|n| n.trim_end().to_string())
        .unwrap_or_default()
}

/// Whether `pid`'s process is on its way out: its leader a zombie (it is
/// one until every other thread has been reaped) or gone altogether.
fn process_exiting(pid: Pid) -> bool {
    let Ok(stat) = std::fs::read_to_string(format!("/proc/{pid}/stat")) else {
        return true;
    };
    // `pid (comm) S ...` — `comm` may itself contain spaces or parentheses.
    let state = stat
        .rsplit_once(')')
        .and_then(|(_, rest)| rest.trim_start().chars().next());
    matches!(state, Some('Z' | 'X') | None)
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
    /// Attaches to every thread of `pid`, leaving only the leader stopped.
    pub fn attach(pid: i32) -> Result<Self, DebugError> {
        let pid = Pid::from_raw(pid);
        let mut debugger = Debugger {
            pid,
            threads: HashMap::new(),
            current: pid,
            breakpoints: HashMap::new(),
            ever_armed: HashSet::new(),
            pending_rearm: None,
            pause_filter: None,
            pause_request: None,
            step_ran_free: false,
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
        // Only the leader: enough to read/patch memory (breakpoints) through,
        // and the rest of the process never notices the attach.
        debugger.stop_threads(|tid| tid == pid)?;
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
    /// valid while it's stopped (see [`is_stopped`](Self::is_stopped)).
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
        // Best effort past this point: a failure here (the process dying
        // under us) must not leave the rest of the threads traced.
        let stopped = self.stop_threads(|_| true);
        for addr in self.breakpoints.keys().copied().collect::<Vec<_>>() {
            let _ = self.clear_breakpoint(addr);
        }
        self.pending_rearm = None;
        for (tid, thread) in self.threads.drain() {
            let _ = ptrace::detach(tid, thread.pending_signal);
        }
        stopped
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
        self.breakpoints.insert(addr, word.to_le_bytes()[0]);
        self.ever_armed.insert(addr);
        self.poke(addr, with_first_byte(word, INT3))?;
        Ok(())
    }

    pub fn clear_breakpoint(&mut self, addr: u64) -> Result<(), DebugError> {
        if let Some(original_byte) = self.breakpoints.remove(&addr) {
            // If we're currently paused right at this breakpoint, its
            // original byte is already restored in memory — poking it
            // again is harmless, but there's no longer anything to step
            // over before the next continue/step.
            if self.pending_rearm.is_some_and(|(a, _)| a == addr) {
                self.pending_rearm = None;
            } else {
                let word = self.peek(addr)?;
                self.poke(addr, with_first_byte(word, original_byte))?;
            }
        }
        Ok(())
    }

    /// Which threads [`pause`](Self::pause) stops, by name (see
    /// [`thread_name`]) — e.g. only the ones that run mod code, so a game's
    /// audio and other housekeeping threads keep going.
    pub fn set_pause_filter(&mut self, filter: impl Fn(&str) -> bool + Send + 'static) {
        self.pause_filter = Some(Box::new(filter));
    }

    /// A flag the host raises (before calling [`interrupt`]) to make an
    /// in-progress [`source_step`](Self::source_step) give up and return
    /// [`StopReason::Interrupted`] — a step can run for as long as the code
    /// it steps over (a call that blocks, or a step that turned into a free
    /// run back to Java), and Break All must still reach it. Interrupts
    /// without the flag (breakpoint edits) are absorbed by the step and the
    /// host's queued work waits for it to finish. The host clears the flag
    /// once it has handled the pause.
    pub fn set_pause_request(&mut self, flag: Arc<AtomicBool>) {
        self.pause_request = Some(flag);
    }

    fn pause_requested(&self) -> bool {
        self.pause_request
            .as_ref()
            .is_some_and(|f| f.load(Ordering::SeqCst))
    }

    /// Turns an [`StopReason::Interrupted`] (only the leader stopped) into
    /// Break All: the threads the pause filter selects are brought to a
    /// stop so they can be inspected and stepped; any others that are
    /// stopped (the leader, usually) are let go again. The first selected
    /// thread becomes current — or the leader stays stopped as current if
    /// no thread matches. The next [`continue_`](Self::continue_) resumes
    /// them all.
    pub fn pause(&mut self) -> Result<(), DebugError> {
        self.step_ran_free = false;
        let pid = self.pid;
        let wanted: HashSet<Pid> = self
            .threads
            .keys()
            .copied()
            .filter(|&tid| {
                self.pause_filter
                    .as_ref()
                    .is_none_or(|f| f(&thread_name(pid, tid)))
            })
            .collect();
        self.stop_threads(|tid| wanted.contains(&tid))?;
        let mut stopped: Vec<Pid> = wanted
            .iter()
            .copied()
            .filter(|tid| self.threads.get(tid).is_some_and(|t| t.stopped))
            .collect();
        stopped.sort_by_key(|t| t.as_raw());
        // The thread paused on a breakpoint is about to be let go: it has
        // to leave the breakpoint (re-armed) first, like on a continue.
        if let Some((_, tid)) = self.pending_rearm {
            if !wanted.contains(&tid) {
                if let Some(Step::Stopped(reason)) = self.step_over_pending_breakpoint()? {
                    if is_terminal(&reason) {
                        return Ok(());
                    }
                }
            }
        }
        if !stopped.contains(&self.current) {
            self.current = stopped.first().copied().unwrap_or(pid);
        }
        let current = self.current;
        for (&tid, thread) in self.threads.iter_mut() {
            if thread.stopped && tid != current && !wanted.contains(&tid) {
                let _ = ptrace::cont(tid, thread.pending_signal.take());
                thread.stopped = false;
            }
        }
        Ok(())
    }

    /// Whether `tid` is stopped (inspectable) right now.
    pub fn is_stopped(&self, tid: Pid) -> bool {
        self.threads.get(&tid).is_some_and(|t| t.stopped)
    }

    /// Resumes every thread and runs until a breakpoint, an [`interrupt`],
    /// or the process ending.
    pub fn continue_(&mut self) -> Result<StopReason, DebugError> {
        self.step_ran_free = false;
        if let Some(Step::Stopped(reason)) = self.step_over_pending_breakpoint()? {
            if is_terminal(&reason) {
                return Ok(reason);
            }
        }
        self.resume_all()?;
        self.run_until_stop()
    }

    /// What's left of a step with nothing more to step through: a run
    /// until the next breakpoint, as [`continue_`](Self::continue_) — and
    /// if an interrupt breaks into it, still that run once resumed.
    fn run_on(&mut self) -> Result<StopReason, DebugError> {
        let reason = self.continue_()?;
        self.step_ran_free = reason == StopReason::Interrupted;
        Ok(reason)
    }

    /// Executes one machine instruction on the current thread; the others
    /// stay as they are. Right after a breakpoint hit, that instruction is the
    /// one the breakpoint sits on. If the thread exits instead, the process
    /// simply runs on, as after [`continue_`](Self::continue_).
    pub fn single_step(&mut self) -> Result<StopReason, DebugError> {
        let step = match self.step_over_pending_breakpoint()? {
            Some(step) => step,
            None => self.step_thread(self.current)?,
        };
        match step {
            Step::Stopped(reason) => Ok(reason),
            Step::Vanished => self.run_on(),
        }
    }

    /// If a thread is paused right at a breakpoint (its original byte
    /// temporarily restored for inspection), execute that one real
    /// instruction on it and re-arm the `0xCC` before doing anything else
    /// the caller asked for — otherwise the same address could never trap
    /// again. `None` if nothing was pending.
    fn step_over_pending_breakpoint(&mut self) -> Result<Option<Step>, DebugError> {
        let Some((addr, tid)) = self.pending_rearm.take() else {
            return Ok(None);
        };
        let step = if self.is_stopped(tid) {
            self.step_thread(tid)?
        } else {
            // Gone already (killed while paused there).
            Step::Vanished
        };
        if let Step::Stopped(reason) = &step {
            if is_terminal(reason) {
                return Ok(Some(step));
            }
        }
        if self.breakpoints.contains_key(&addr) {
            self.rearm(addr)?;
        }
        Ok(Some(step))
    }

    /// Writes the `0xCC` back at `addr` — through `/proc/<pid>/mem` if no
    /// thread is stopped to write through any more (the one that was paused
    /// there vanished). A process too far gone to write to is left to the
    /// run that follows, which reports its end.
    fn rearm(&mut self, addr: u64) -> Result<(), DebugError> {
        if !self.threads.values().any(|t| t.stopped) {
            use std::os::unix::fs::FileExt;
            if let Ok(mem) = std::fs::OpenOptions::new()
                .write(true)
                .open(format!("/proc/{}/mem", self.pid))
            {
                let _ = mem.write_all_at(&[INT3], addr);
            }
            return Ok(());
        }
        let result = self
            .peek(addr)
            .and_then(|word| self.poke(addr, with_first_byte(word, INT3)));
        match result {
            Err(DebugError::Ptrace(e)) if vanished(&e) => Ok(()),
            other => other,
        }
    }

    /// `waitpid` on one thread — except that a leader whose process is
    /// exiting never reports while other threads are still unreaped (which
    /// only waiting on *them* does), so that returns `None` instead of
    /// blocking forever.
    fn wait_thread(&self, tid: Pid) -> nix::Result<Option<WaitStatus>> {
        if tid != self.pid {
            return waitpid(tid, wait_flags()).map(Some);
        }
        loop {
            match waitpid(tid, wait_flags().map(|f| f | WaitPidFlag::WNOHANG))? {
                WaitStatus::StillAlive => {
                    if process_exiting(self.pid) {
                        return Ok(None);
                    }
                    std::thread::sleep(std::time::Duration::from_micros(200));
                }
                status => return Ok(Some(status)),
            }
        }
    }

    /// One instruction on `tid`, absorbing stops that aren't the step
    /// itself: a signal arriving mid-step is delivered on the retried step
    /// (entering the handler, like gdb), a stale interrupt stop is retried.
    fn step_thread(&mut self, tid: Pid) -> Result<Step, DebugError> {
        let mut signal = self
            .threads
            .get_mut(&tid)
            .and_then(|t| t.pending_signal.take());
        loop {
            match ptrace::step(tid, signal.take()) {
                Ok(()) => {}
                // Killed while stopped: its exit is reaped later, by
                // whatever runs next.
                Err(e) if vanished(&e) => {
                    self.forget_thread(tid);
                    return Ok(Step::Vanished);
                }
                Err(e) => return Err(e.into()),
            }
            self.mark(tid, false);
            let Some(status) = self.wait_thread(tid)? else {
                return Ok(Step::Vanished);
            };
            match status {
                WaitStatus::Exited(_, code) => {
                    return Ok(self.thread_gone(tid, StopReason::Exited(code)))
                }
                WaitStatus::Signaled(_, sig, _) => {
                    return Ok(self.thread_gone(tid, StopReason::Killed(sig)));
                }
                WaitStatus::Stopped(_, Signal::SIGTRAP) => {
                    self.mark(tid, true);
                    return Ok(Step::Stopped(StopReason::Signal(Signal::SIGTRAP)));
                }
                WaitStatus::Stopped(_, Signal::SIGSTOP) => {}
                WaitStatus::Stopped(_, sig) => signal = Some(sig),
                WaitStatus::PtraceEvent(_, _, event) if event == nix::libc::PTRACE_EVENT_CLONE => {
                    self.adopt_clone(tid);
                }
                _ => {}
            }
        }
    }

    /// A thread exiting isn't the process exiting unless it's the leader.
    fn thread_gone(&mut self, tid: Pid, reason: StopReason) -> Step {
        self.forget_thread(tid);
        if tid == self.pid || self.threads.is_empty() {
            Step::Stopped(reason)
        } else {
            Step::Vanished
        }
    }

    /// Stops tracking `tid`; if it was current, another stopped thread
    /// (else the leader) takes over.
    fn forget_thread(&mut self, tid: Pid) {
        // A `pending_rearm` naming it stays: the breakpoint still needs its
        // `0xCC` back (see `step_over_pending_breakpoint`).
        self.threads.remove(&tid);
        if self.current == tid {
            let pid = self.pid;
            self.current = self
                .threads
                .iter()
                .filter(|(_, t)| t.stopped)
                .map(|(&t, _)| t)
                .min_by_key(|&t| (t != pid, t.as_raw()))
                .unwrap_or(pid);
        }
    }

    fn mark(&mut self, tid: Pid, stopped: bool) {
        self.threads.entry(tid).or_default().stopped = stopped;
    }

    /// `tid` reported a clone event: track the new thread (it starts in a
    /// ptrace-stop of its own, which `run_until_stop` resumes). If `tid`
    /// vanished before saying which thread it made, the child is still
    /// picked up from its own first stop.
    fn adopt_clone(&mut self, tid: Pid) -> Option<Pid> {
        let child = Pid::from_raw(ptrace::getevent(tid).ok()? as i32);
        self.threads.entry(child).or_default();
        Some(child)
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

    /// Brings the running threads `want` selects to a ptrace-stop (threads
    /// born meanwhile from one of them too). A thread that had already
    /// trapped on a breakpoint is rewound onto it, so it simply traps again
    /// (and gets reported) once resumed.
    fn stop_threads(&mut self, want: impl Fn(Pid) -> bool) -> Result<(), DebugError> {
        let running: Vec<Pid> = self
            .threads
            .iter()
            .filter(|(&tid, t)| !t.stopped && want(tid))
            .map(|(&tid, _)| tid)
            .collect();
        for &tid in &running {
            let _ = ptrace::interrupt(tid);
        }
        let mut waiting = running;
        while let Some(tid) = waiting.pop() {
            let status = match self.wait_thread(tid) {
                Ok(Some(s)) => s,
                // The process is exiting: nothing to stop.
                Ok(None) => continue,
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
                WaitStatus::Stopped(_, Signal::SIGTRAP) => match self.rewind_if_trapped(tid) {
                    Ok(_) => self.mark(tid, true),
                    // Killed meanwhile; its exit is reaped later.
                    Err(DebugError::Ptrace(e)) if vanished(&e) => self.mark(tid, false),
                    Err(e) => return Err(e),
                },
                // Our own `interrupt` — swallowed, never delivered.
                WaitStatus::Stopped(_, Signal::SIGSTOP) => self.mark(tid, true),
                WaitStatus::Stopped(_, sig) => {
                    let thread = self.threads.entry(tid).or_default();
                    thread.pending_signal = Some(sig);
                    thread.stopped = true;
                }
                WaitStatus::PtraceEvent(_, _, event) => {
                    if event == nix::libc::PTRACE_EVENT_CLONE {
                        let known = self.threads.keys().copied().collect::<HashSet<_>>();
                        if let Some(child) = self.adopt_clone(tid) {
                            if !known.contains(&child) {
                                waiting.push(child);
                            }
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
                    self.forget_thread(tid);
                    if tid == self.pid {
                        return Ok(StopReason::Exited(code));
                    }
                }
                WaitStatus::Signaled(_, sig, _) => {
                    self.forget_thread(tid);
                    if tid == self.pid {
                        return Ok(StopReason::Killed(sig));
                    }
                }
                WaitStatus::PtraceEvent(_, _, event) => {
                    if event == nix::libc::PTRACE_EVENT_CLONE {
                        self.adopt_clone(tid);
                    } else {
                        // A new thread's first stop, a stale interrupt, or
                        // a group-stop: just keep it running.
                        self.threads.entry(tid).or_default();
                    }
                    let _ = ptrace::cont(tid, None);
                }
                WaitStatus::Stopped(_, Signal::SIGTRAP) => {
                    self.mark(tid, true);
                    let trapped = match self.rewind_if_trapped(tid) {
                        Ok(trapped) => trapped,
                        // Killed right after trapping (the process
                        // exiting): its exit comes next.
                        Err(DebugError::Ptrace(e)) if vanished(&e) => {
                            self.mark(tid, false);
                            continue;
                        }
                        Err(e) => return Err(e),
                    };
                    match trapped {
                        Some(addr) if self.breakpoints.contains_key(&addr) => {
                            match self.report_breakpoint(tid, addr) {
                                Err(DebugError::Ptrace(e)) if vanished(&e) => {
                                    self.mark(tid, false);
                                }
                                reported => return reported,
                            }
                        }
                        // A breakpoint cleared before this trap surfaced:
                        // rewound onto the original instruction, carry on.
                        Some(_) => {
                            self.mark(tid, false);
                            let _ = ptrace::cont(tid, None);
                        }
                        // Not ours — the tracee's own SIGTRAP.
                        None => {
                            self.mark(tid, false);
                            let _ = ptrace::cont(tid, Signal::SIGTRAP);
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
        // Restore the real instruction byte, but do NOT step past it yet —
        // the thread stays genuinely paused at `addr` until the next
        // continue/step, so backtraces/inspection see the real location.
        let original_byte = self.breakpoints[&addr];
        let word = ptrace::read(tid, addr as ptrace::AddressType)?;
        ptrace::write(
            tid,
            addr as ptrace::AddressType,
            with_first_byte(word, original_byte),
        )?;
        self.current = tid;
        self.pending_rearm = Some((addr, tid));
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
    /// line" checks during stepping: the innermost *user* line at `rip`
    /// (see [`user_location`]) — `None` in library code or code without
    /// lines, which stepping never stops in.
    fn line_key(&self, symbols: &SymbolTable, base: u64) -> Option<LineKey> {
        match self.position(symbols, base) {
            Position::User(key) => Some(key),
            Position::NoLine | Position::Foreign => None,
        }
    }

    /// What kind of code `rip` is in, for stepping.
    fn position(&self, symbols: &SymbolTable, base: u64) -> Position {
        let Some(offset) = self.registers().ok().and_then(|r| r.rip.checked_sub(base)) else {
            return Position::Foreign;
        };
        let chain = symbols.resolve_inlined(offset);
        if let Some(loc) = chain
            .iter()
            .find(|loc| loc.line != 0 && !is_library_source(&loc.file))
        {
            return Position::User((loc.file.clone(), loc.line));
        }
        if chain.is_empty() || chain.iter().any(|loc| is_library_source(&loc.file)) {
            Position::Foreign
        } else {
            Position::NoLine
        }
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
                // Break All gets through; breakpoint edits wait for the
                // step to finish — unless the stepping thread is gone, which
                // leaves this an ordinary run the edits mustn't wait on.
                StopReason::Interrupted if self.pause_requested() => break Ok(reason),
                StopReason::Interrupted if !self.threads.contains_key(&origin) => {
                    self.step_ran_free = true;
                    break Ok(reason);
                }
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
    ///
    /// A step that leaves user code with no user caller to return to (a JNI
    /// entry point returning to Java) turns into a free run, like
    /// [`continue_`](Self::continue_) — and like it, returns
    /// [`StopReason::Interrupted`] for any [`interrupt`], pause request or
    /// not. What's left of such a step is just that run: resume it with
    /// `continue_` — or `source_step` again, which knows and does the same.
    pub fn source_step(
        &mut self,
        kind: StepKind,
        symbols: &SymbolTable,
        base: u64,
    ) -> Result<StopReason, DebugError> {
        // A step that had already turned into a free run (see
        // `leave_foreign_code`), resumed after an interrupt broke into it —
        // its thread may well be gone — or a thread that isn't stopped:
        // all that's left is to run on.
        if std::mem::take(&mut self.step_ran_free) || !self.is_stopped(self.current) {
            return self.run_on();
        }
        // Paused outside user code (a Break All landing in the JVM or in
        // `std`): there's nothing to step through, only a way back to it.
        let Some(start) = self.line_key(symbols, base) else {
            return self.leave_foreign_code(symbols, base);
        };
        if kind == StepKind::Out {
            return self.step_out(symbols, base);
        }
        // A generous ceiling so a pathological step (e.g. into a huge
        // library call whose return we somehow miss) can't spin forever —
        // reaching it just stops where we are, same as any other trap.
        const MAX_STEPS: usize = 5_000_000;

        for _ in 0..MAX_STEPS {
            if self.pause_requested() {
                return Ok(StopReason::Interrupted);
            }
            let rip = self.registers()?.rip;
            let (is_call, len) = self.decode_at(rip);

            let reason = if is_call && kind == StepKind::Over {
                self.run_to(rip + len)?
            } else if is_call && kind == StepKind::Into {
                let stepped = self.single_step()?;
                // Ended, or the thread exited and the process ran on.
                if stepped != StopReason::Signal(Signal::SIGTRAP) {
                    return Ok(stepped);
                }
                // If we descended into library code (std, a dependency, or
                // anything without lines), run straight back out to the
                // instruction after the call rather than crawling through
                // it instruction by instruction — VS's Just My Code.
                if self.line_key(symbols, base).is_none() {
                    let back = self.run_to(rip + len)?;
                    if is_terminal(&back)
                        || matches!(back, StopReason::Breakpoint(_) | StopReason::Interrupted)
                    {
                        return Ok(back);
                    }
                }
                StopReason::Signal(Signal::SIGTRAP)
            } else {
                self.single_step()?
            };

            match reason {
                StopReason::Exited(_) | StopReason::Killed(_) => return Ok(reason),
                StopReason::Breakpoint(_) | StopReason::Interrupted => return Ok(reason),
                StopReason::Signal(Signal::SIGTRAP) => {}
                StopReason::Signal(other) => return Ok(StopReason::Signal(other)),
            }

            // Stepped off the end of user code — returned into the JVM that
            // called the mod, into the `std` function that called a closure
            // (`OnceLock::get_or_init`), or tail-jumped into a library.
            // Crawling through that an instruction at a time is what made
            // such a step look hung.
            let cur = match self.position(symbols, base) {
                Position::User(key) => key,
                // A compiler-generated row (line 0) of user code: keep going.
                Position::NoLine => continue,
                Position::Foreign => {
                    // Back in the user caller (mid-statement, right after
                    // its call), where stepping goes on as after any
                    // return; anything else ends the step.
                    let back = self.leave_foreign_code(symbols, base)?;
                    if back != StopReason::Signal(Signal::SIGTRAP) {
                        return Ok(back);
                    }
                    match self.line_key(symbols, base) {
                        Some(key) => key,
                        None => return Ok(back),
                    }
                }
            };
            // Only where a statement begins: mid-statement rows are the
            // compiler interleaving lines, not the code moving on.
            let stop = cur != start
                && self
                    .registers()
                    .is_ok_and(|r| symbols.is_statement_start(r.rip.wrapping_sub(base)));
            if stop {
                return Ok(StopReason::Signal(Signal::SIGTRAP));
            }
        }
        Ok(StopReason::Signal(Signal::SIGTRAP))
    }

    /// Shift+F11: runs to the return address of the current frame (found by
    /// CFI), then on out of any library code to the nearest user caller.
    fn step_out(&mut self, symbols: &SymbolTable, base: u64) -> Result<StopReason, DebugError> {
        let frames = self.backtrace_with(symbols, base, 2)?;
        let Some(caller) = frames.get(1) else {
            return self.run_on();
        };
        let back = self.run_to(caller.pc().wrapping_add(1))?;
        if back != StopReason::Signal(Signal::SIGTRAP) {
            return Ok(back);
        }
        if self.line_key(symbols, base).is_none() {
            return self.leave_foreign_code(symbols, base);
        }
        Ok(back)
    }

    /// Gets back from library code (or code without lines), like Visual
    /// Studio's Just My Code: runs to the nearest caller in user code
    /// (stopping there, right after its call), or — when nothing up the
    /// stack is user code, as when a JNI entry point returns to Java —
    /// simply continues until the next breakpoint.
    fn leave_foreign_code(
        &mut self,
        symbols: &SymbolTable,
        base: u64,
    ) -> Result<StopReason, DebugError> {
        let caller = self
            .backtrace_with(symbols, base, CALLER_SEARCH_DEPTH)?
            .into_iter()
            .skip(1)
            .find(|frame| {
                frame
                    .pc()
                    .checked_sub(base)
                    .and_then(|offset| user_location(symbols, offset))
                    .is_some()
            });
        match caller {
            Some(frame) => self.run_to(frame.pc().wrapping_add(1)),
            None => self.run_on(),
        }
    }

    /// The current thread's full register set as `yog-symbols` frame 0.
    pub fn frame_regs(&self) -> Result<FrameRegs, DebugError> {
        let r = self.registers()?;
        Ok(FrameRegs::from_dwarf_order([
            r.rax, r.rdx, r.rcx, r.rbx, r.rsi, r.rdi, r.rbp, r.rsp, r.r8, r.r9, r.r10, r.r11,
            r.r12, r.r13, r.r14, r.r15, r.rip,
        ]))
    }

    /// Backtrace of the current thread through the mod's call frame info
    /// (see [`SymbolTable::backtrace`]): innermost first, at most `max`
    /// frames, each with every register CFI could recover — outer frames'
    /// pc is their return address minus one, ready for
    /// [`SymbolTable::resolve_addr`] / [`SymbolTable::read_locals`]. Walks
    /// through the frame-pointer-less `std` that [`Self::backtrace`] can't.
    pub fn backtrace_with(
        &self,
        symbols: &SymbolTable,
        base: u64,
        max: usize,
    ) -> Result<Vec<FrameRegs>, DebugError> {
        let regs = self.frame_regs()?;
        Ok(symbols.backtrace(&regs, base, &PeekMemory(self), max))
    }

    /// Frame-pointer-walk backtrace: return addresses only, in innermost-
    /// first order, capped to avoid running away on a corrupt/omitted frame
    /// chain. Callers resolve each address through `yog-symbols` after
    /// subtracting the relevant module's load base.
    ///
    /// Relies on frame pointers, which the prebuilt `std` doesn't keep —
    /// prefer [`Self::backtrace_with`], which unwinds by CFI.
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

/// Tracee memory a word at a time through `PTRACE_PEEKDATA`.
struct PeekMemory<'a>(&'a Debugger);

impl TargetMemory for PeekMemory<'_> {
    fn read(&self, addr: u64, buf: &mut [u8]) -> bool {
        for (i, chunk) in buf.chunks_mut(8).enumerate() {
            let Ok(word) = self.0.peek(addr.wrapping_add(i as u64 * 8)) else {
                return false;
            };
            chunk.copy_from_slice(&word.to_ne_bytes()[..chunk.len()]);
        }
        true
    }
}
