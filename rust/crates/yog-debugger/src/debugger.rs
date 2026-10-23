//! Native breakpoint debugging of a running process — byte-patch
//! breakpoints (`int3`, `brk`), stops handled thread by thread, source
//! steps — on whichever system's backend ([`crate::target`]). Address
//! resolution against source is `yog-symbols`'; this module only knows
//! raw addresses in the attached process.

use std::collections::{HashMap, HashSet};
use yog_sync::Arc;
use yog_sync::atomic::{AtomicBool, Ordering};

use crate::arch::BREAKPOINT;
use crate::platform::Process;
use crate::target::{Module, OsError, Target, Tid};

mod source;
mod threads;

type LineKey = (std::path::PathBuf, u32);

/// The most instructions a source step runs: a pathological step (into a
/// huge library call whose return is somehow missed) stops where it is.
const MAX_SOURCE_STEPS: usize = 5_000_000;

/// How far up the stack [`Debugger::leave_foreign_code`] looks for a user
/// caller to return to.
const CALLER_SEARCH_DEPTH: usize = 64;

/// Where a source step's instruction landed.
enum Landed {
    /// On a user line.
    Line(LineKey),
    /// Still within a line (a compiler-generated row).
    Within,
    /// The step ends here.
    Stop(StopReason),
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
    Os(OsError),
    /// The debugged process stopped for a reason other than one we asked for.
    UnexpectedStop(String),
}

impl std::fmt::Display for DebugError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DebugError::Os(e) => e.fmt(f),
            DebugError::UnexpectedStop(s) => write!(f, "unexpected stop: {s}"),
        }
    }
}

impl std::error::Error for DebugError {}

impl From<OsError> for DebugError {
    fn from(e: OsError) -> Self {
        DebugError::Os(e)
    }
}

impl DebugError {
    /// Whether the system refused us the process — attaching needs a right
    /// we haven't got (each system its own: Linux's ptrace scope, macOS's
    /// debugger entitlement and the target's `get-task-allow`).
    pub fn denied(&self) -> bool {
        matches!(self, DebugError::Os(OsError { denied: true, .. }))
    }
}

/// Whether `e` says the thread it named is gone (killed while stopped):
/// one such thread mustn't end the whole session — its end is reported later.
fn vanished(e: &DebugError) -> bool {
    matches!(e, DebugError::Os(OsError { gone: true, .. }))
}

/// Whether a stop means the process is gone (exited or killed) — a stepping
/// loop must bail immediately rather than trying to read its registers.
fn is_terminal(reason: &StopReason) -> bool {
    matches!(reason, StopReason::Exited(_) | StopReason::Killed(_))
}

/// What became of single-stepping one thread.
enum Step {
    /// The step completed, or the process ended.
    Stopped(StopReason),
    /// The thread exited (or was killed) instead of completing the step;
    /// it's no longer tracked, and `current` moved to another thread.
    Vanished,
}

/// Why the process stopped running, after a `continue_`/`single_step`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StopReason {
    /// Hit one of our own breakpoints — the pc is at the breakpoint
    /// address. The thread that hit it is [`Debugger::current_thread`];
    /// the others run on.
    Breakpoint(u64),
    /// A single step (or a source step) completed.
    Step,
    /// [`interrupt`](crate::interrupt) reached the debugger: enough to
    /// set/clear breakpoints; [`pause`](Debugger::pause) makes it Break All.
    Interrupted,
    Exited(i32),
    /// Killed by this signal (Linux) or code.
    Killed(i32),
}

/// One live debug session against a running process: every thread is
/// followed (threads created later too), the process's own signals and
/// exceptions — the JVM relies on them for safepoints and implicit null
/// checks — go straight back to it, and a breakpoint hit stops only the
/// thread that hit it — the rest of the process (its sound, its server
/// thread, ...) keeps running, like gdb's non-stop mode.
/// [`pause`](Debugger::pause) stops only the threads its filter selects.
pub struct Debugger {
    process: Process,
    /// Every thread, and whether it's stopped (inspectable, needs resuming).
    threads: HashMap<Tid, bool>,
    /// The stopped thread registers, backtraces and steps refer to — the
    /// one that last hit a breakpoint (the leader right after attach).
    current: Tid,
    /// Runtime address -> the original bytes our breakpoint replaced.
    breakpoints: HashMap<u64, Vec<u8>>,
    /// Every address ever armed. A thread can trap on a breakpoint that's
    /// cleared before the trap is reported; it must still be recognised
    /// as ours rather than handed to the process (which would kill the JVM).
    ever_armed: HashSet<u64>,
    /// Set right after a breakpoint hit: the address whose original bytes
    /// are (temporarily) restored so the thread that hit it sits paused
    /// exactly at the breakpoint for inspection, and that thread. The next
    /// `continue_`/`single_step` must step *that thread* over it and re-arm
    /// the breakpoint first, or it would never fire again.
    pending_rearm: Option<(u64, Tid)>,
    /// Which threads (by name) [`pause`](Debugger::pause) stops; all when unset.
    pause_filter: Option<PauseFilter>,
    /// Set by the host (see [`Debugger::set_pause_request`]) when an
    /// [`interrupt`](crate::interrupt) means Break All rather than a breakpoint edit.
    pause_request: Option<Arc<AtomicBool>>,
    /// The last [`source_step`](Debugger::source_step) had turned into a
    /// free run when an interrupt broke into it: there's no step left to
    /// resume, only that run.
    step_ran_free: bool,
}

type PauseFilter = Box<dyn Fn(&str) -> bool + Send>;

impl Debugger {
    /// Attaches to every thread of `pid`, leaving only the leader stopped.
    pub fn attach(pid: u32) -> Result<Self, DebugError> {
        let (process, tids) = Process::attach(pid)?;
        let leader = process.leader();
        let mut debugger = Debugger {
            process,
            threads: tids.into_iter().map(|t| (t, false)).collect(),
            current: leader,
            breakpoints: HashMap::new(),
            ever_armed: HashSet::new(),
            pending_rearm: None,
            pause_filter: None,
            pause_request: None,
            step_ran_free: false,
        };
        // Only the leader: the rest of the process never notices the attach.
        debugger.stop_threads(|tid| tid == leader)?;
        Ok(debugger)
    }

    pub fn pid(&self) -> u32 {
        self.process.pid()
    }

    /// The thread registers, backtraces and steps currently refer to.
    pub fn current_thread(&self) -> Tid {
        self.current
    }

    /// Every followed thread id, leader first.
    pub fn threads(&self) -> Vec<Tid> {
        let leader = self.process.leader();
        let mut tids: Vec<Tid> = self.threads.keys().copied().collect();
        tids.sort_by_key(|&t| (t != leader, t));
        tids
    }

    /// A thread's name (`Render thread`), as the system knows it.
    pub fn thread_name(&self, tid: Tid) -> String {
        self.process.thread_name(tid)
    }

    /// The process's loaded modules.
    pub fn modules(&self) -> Vec<Module> {
        self.process.modules()
    }

    /// The load base of the module whose file is `path` (compared by
    /// canonical path, falling back to the file name: the process may see
    /// another mount namespace).
    pub fn module_base(&self, path: &std::path::Path) -> Option<u64> {
        let canonical = path.canonicalize().ok();
        let name = path.file_name();
        self.modules()
            .into_iter()
            .find(|m| {
                canonical.as_deref() == Some(&m.path)
                    || name.is_some_and(|n| m.path.file_name() == Some(n))
            })
            .map(|m| m.base)
    }

    /// Makes `tid` the thread registers/backtraces/steps refer to. Only
    /// valid while it's stopped (see [`is_stopped`](Self::is_stopped)).
    pub fn select_thread(&mut self, tid: Tid) -> Result<(), DebugError> {
        if !self.is_stopped(tid) {
            return Err(DebugError::UnexpectedStop(format!(
                "thread {tid} isn't stopped"
            )));
        }
        self.current = tid;
        Ok(())
    }

    pub fn detach(&mut self) -> Result<(), DebugError> {
        // Best effort past this point: a failure here (the process dying
        // under us) must not leave the rest of the threads held.
        let stopped = self.stop_threads(|_| true);
        for addr in self.breakpoints.keys().copied().collect::<Vec<_>>() {
            let _ = self.clear_breakpoint(addr);
        }
        self.pending_rearm = None;
        let held: Vec<Tid> = self
            .threads
            .drain()
            .filter(|&(_, stopped)| stopped)
            .map(|(t, _)| t)
            .collect();
        self.process.detach(&held);
        stopped
    }

    /// Arms a breakpoint at a real runtime address (a module's load base +
    /// a `yog-symbols`-resolved offset). No-op if already armed there.
    pub fn set_breakpoint(&mut self, addr: u64) -> Result<(), DebugError> {
        if self.breakpoints.contains_key(&addr) {
            return Ok(());
        }
        let mut original = vec![0; BREAKPOINT.len()];
        if !self.process.read(addr, &mut original) {
            return Err(DebugError::UnexpectedStop(format!(
                "{addr:#x} isn't readable"
            )));
        }
        self.breakpoints.insert(addr, original);
        self.ever_armed.insert(addr);
        self.process.claim(addr);
        self.process.write(addr, BREAKPOINT)?;
        Ok(())
    }

    pub fn clear_breakpoint(&mut self, addr: u64) -> Result<(), DebugError> {
        if let Some(original) = self.breakpoints.remove(&addr) {
            // Paused right at this breakpoint: its original bytes are
            // already back, and there's nothing to step over any more.
            if self.pending_rearm.is_some_and(|(a, _)| a == addr) {
                self.pending_rearm = None;
            } else {
                self.process.write(addr, &original)?;
            }
        }
        Ok(())
    }

    /// Which threads [`pause`](Self::pause) stops, by name (see
    /// [`thread_name`](Self::thread_name)) — e.g. only the ones that run mod
    /// code, so a game's audio and other housekeeping threads keep going.
    pub fn set_pause_filter(&mut self, filter: impl Fn(&str) -> bool + Send + 'static) {
        self.pause_filter = Some(Box::new(filter));
    }

    /// A flag the host raises (before calling [`interrupt`](crate::interrupt))
    /// to make an in-progress [`source_step`](Self::source_step) give up and
    /// return [`StopReason::Interrupted`] — a step can run for as long as the
    /// code it steps over (a call that blocks, or a step that turned into a
    /// free run back to Java), and Break All must still reach it. Interrupts
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

    /// Turns an [`StopReason::Interrupted`] into Break All: the threads the
    /// pause filter selects are brought to a stop so they can be inspected
    /// and stepped; any others that are stopped (the leader, usually) are
    /// let go again. The first selected thread becomes current — or the
    /// leader stays stopped as current if no thread matches. The next
    /// [`continue_`](Self::continue_) resumes them all.
    pub fn pause(&mut self) -> Result<(), DebugError> {
        self.step_ran_free = false;
        let leader = self.process.leader();
        let wanted: HashSet<Tid> = self
            .threads
            .keys()
            .copied()
            .filter(|&tid| {
                self.pause_filter
                    .as_ref()
                    .is_none_or(|f| f(&self.process.thread_name(tid)))
            })
            .collect();
        self.stop_threads(|tid| wanted.contains(&tid))?;
        let mut stopped: Vec<Tid> = wanted
            .iter()
            .copied()
            .filter(|&tid| self.is_stopped(tid))
            .collect();
        stopped.sort_unstable();
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
            if stopped.is_empty() {
                self.stop_threads(|tid| tid == leader)?;
            }
            self.current = stopped.first().copied().unwrap_or(leader);
        }
        let current = self.current;
        let release: Vec<Tid> = self
            .threads
            .iter()
            .filter(|&(&tid, &s)| s && tid != current && !wanted.contains(&tid))
            .map(|(&tid, _)| tid)
            .collect();
        for tid in release {
            let _ = self.process.resume(tid);
            self.mark(tid, false);
        }
        Ok(())
    }

    /// Whether `tid` is stopped (inspectable) right now.
    pub fn is_stopped(&self, tid: Tid) -> bool {
        self.threads.get(&tid).copied().unwrap_or(false)
    }

    /// Resumes every thread and runs until a breakpoint, an interrupt, or
    /// the process ending.
    pub fn continue_(&mut self) -> Result<StopReason, DebugError> {
        self.step_ran_free = false;
        if let Some(Step::Stopped(reason)) = self.step_over_pending_breakpoint()?
            && is_terminal(&reason)
        {
            return Ok(reason);
        }
        self.resume_all();
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
    /// stay as they are. Right after a breakpoint hit, that instruction is
    /// the one the breakpoint sits on. If the thread exits instead, the
    /// process simply runs on, as after [`continue_`](Self::continue_).
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

    /// If a thread is paused right at a breakpoint (its original bytes
    /// temporarily restored for inspection), execute that one real
    /// instruction on it and re-arm the breakpoint before doing anything
    /// else the caller asked for — otherwise the same address could never
    /// trap again. `None` if nothing was pending.
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
            // A process too far gone to write to is left to the run that
            // follows, which reports its end.
            match self.process.write(addr, BREAKPOINT) {
                Err(e) if !e.gone => return Err(e.into()),
                _ => {}
            }
        }
        Ok(Some(step))
    }
}
