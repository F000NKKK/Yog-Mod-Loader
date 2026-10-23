//! The debugged process as the operating system lets a debugger drive it —
//! one backend per system (`platform::Process`): Linux's `ptrace`, the
//! Windows debug API, macOS's Mach exception ports. They all keep the same
//! model, the one [`crate::Debugger`] is written against:
//!
//! - **non-stop**: a thread that traps stays stopped, every other runs on;
//!   threads are stopped and resumed one by one;
//! - **memory** is read and written whenever (no thread needs to be stopped);
//! - **events** come out of one blocking [`wait`](Target::wait): a trap, a
//!   requested stop, an [`interrupt`](crate::interrupt), threads coming and
//!   going, the process ending. The process's own traps and signals (not a
//!   breakpoint of ours, not a step) go back to it inside the backend.

use std::path::PathBuf;

use yog_symbols::FrameRegs;

/// A thread of the debugged process, as its system numbers it.
pub type Tid = u64;

/// A module the process has loaded: where, and from which file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Module {
    pub base: u64,
    pub path: PathBuf,
}

/// A stopped thread's registers: as frame 0 of a backtrace (DWARF
/// numbering), and by name as the debugger shows them.
#[derive(Debug, Clone)]
pub struct Regs {
    pub frame: FrameRegs,
    pub named: Vec<(&'static str, u64)>,
}

impl Regs {
    pub fn pc(&self) -> u64 {
        self.frame.pc()
    }

    pub fn sp(&self) -> u64 {
        self.frame.sp().unwrap_or(0)
    }

    pub fn fp(&self) -> u64 {
        self.frame.fp().unwrap_or(0)
    }
}

/// What [`wait`](Target::wait) saw.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Event {
    /// The thread trapped — a breakpoint instruction (one of the addresses
    /// [`claim`](Target::claim)ed) or the end of a [`step`](Target::step).
    /// It's stopped.
    Trap(Tid),
    /// A thread [`stop`](Target::stop) was asked of stopped.
    Stopped(Tid),
    /// [`interrupt`](crate::interrupt) reached the debugger; the thread it
    /// stopped, if the system stops one for it (Linux stops the leader).
    Interrupted(Option<Tid>),
    /// A new thread; `stopped` when it starts stopped (Linux's clone event
    /// stops the parent — `parent` — which is resumed or stepped as it was).
    Spawned { child: Tid, parent: Option<Tid> },
    /// A thread ended.
    Gone(Tid),
    /// A thread that can't be waited on any more: the process is ending.
    Vanished(Tid),
    /// The process ended with this code, or was killed by this signal.
    Exited(i32),
    Killed(i32),
}

/// Why something the system was asked failed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OsError {
    pub what: &'static str,
    pub message: String,
    /// The thread (or process) it named isn't there any more.
    pub gone: bool,
}

impl std::fmt::Display for OsError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {}", self.what, self.message)
    }
}

/// One debugged process: what every backend offers.
pub trait Target: Sized {
    /// Attaches to process `pid`; its threads, the leader first, all running.
    fn attach(pid: u32) -> Result<(Self, Vec<Tid>), OsError>;
    fn pid(&self) -> u32;
    /// The thread that stands for the process (Linux's leader, the main thread).
    fn leader(&self) -> Tid;
    /// A thread's name (`Render thread`); empty when it has none or is gone.
    fn thread_name(&self, tid: Tid) -> String;
    /// Asks running `tid` to stop; it reports [`Event::Stopped`] (or a trap it hit first).
    fn stop(&mut self, tid: Tid) -> Result<(), OsError>;
    /// Lets stopped `tid` run.
    fn resume(&mut self, tid: Tid) -> Result<(), OsError>;
    /// Lets stopped `tid` run one instruction; it reports [`Event::Trap`].
    fn step(&mut self, tid: Tid) -> Result<(), OsError>;
    /// The next event — of `only` (and the process's end), others kept for later.
    fn wait(&mut self, only: Option<Tid>) -> Result<Event, OsError>;
    fn regs(&self, tid: Tid) -> Result<Regs, OsError>;
    fn set_pc(&mut self, tid: Tid, pc: u64) -> Result<(), OsError>;
    /// Fills `buf` from `addr`; `false` if any of it is unreadable.
    fn read(&self, addr: u64, buf: &mut [u8]) -> bool;
    /// Writes `bytes` at `addr` (code too: breakpoints).
    fn write(&mut self, addr: u64, bytes: &[u8]) -> Result<(), OsError>;
    /// Marks `addr` as holding a breakpoint of ours: its traps are ours to
    /// report, not the process's own.
    fn claim(&mut self, addr: u64);
    /// The process's loaded modules.
    fn modules(&self) -> Vec<Module>;
    /// Lets the process go, `stopped` threads resumed.
    fn detach(&mut self, stopped: &[Tid]);
}
