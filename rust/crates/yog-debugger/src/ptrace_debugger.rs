//! Real native breakpoint debugging via Linux `ptrace` — classic INT3
//! (`0xCC`) byte-patch breakpoints, `waitpid`-driven stop handling, and a
//! frame-pointer backtrace. Address resolution against source is delegated
//! entirely to `yog-symbols`; this module only knows about raw addresses in
//! the attached process.

use std::collections::{HashMap, HashSet};
use yog_sync::Arc;
use yog_sync::atomic::{AtomicBool, Ordering};

use nix::sys::ptrace;
use nix::sys::signal::Signal;
use nix::sys::wait::WaitPidFlag;
use nix::unistd::Pid;

mod source;
mod threads;

type LineKey = (std::path::PathBuf, u32);

/// The x86 `INT3` opcode a breakpoint patches in.
const INT3: u8 = 0xCC;

/// The most instructions a source step runs: a pathological step (into a
/// huge library call whose return is somehow missed) stops where it is.
const MAX_SOURCE_STEPS: usize = 5_000_000;

/// Where a source step's instruction landed.
enum Landed {
    /// On a user line.
    Line(LineKey),
    /// Still within a line (a compiler-generated row).
    Within,
    /// The step ends here.
    Stop(StopReason),
}

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
        if let Some((_, tid)) = self.pending_rearm
            && !wanted.contains(&tid)
            && let Some(Step::Stopped(reason)) = self.step_over_pending_breakpoint()?
            && is_terminal(&reason)
        {
            return Ok(());
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
        if let Some(Step::Stopped(reason)) = self.step_over_pending_breakpoint()?
            && is_terminal(&reason)
        {
            return Ok(reason);
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
        if let Step::Stopped(reason) = &step
            && is_terminal(reason)
        {
            return Ok(Some(step));
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
}
