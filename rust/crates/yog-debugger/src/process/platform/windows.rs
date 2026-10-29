//! Windows: the debug API. Every debug event freezes the whole process
//! until it's continued; a thread that should stay stopped (a trap, a
//! stop asked for) is suspended before that, so the rest runs on —
//! non-stop, as on Linux. The program's own exceptions (the JVM's
//! safepoints and implicit null checks) go back to it unhandled.

use std::collections::{HashMap, HashSet, VecDeque};
use substrate_lock::Locked;

use std::sync::Mutex;
use windows_sys::Win32::Foundation::{CloseHandle, HANDLE, NTSTATUS, WAIT_OBJECT_0};
use windows_sys::Win32::System::Diagnostics::Debug::{
    DebugActiveProcess, DebugActiveProcessStop, DebugBreakProcess, DebugSetProcessKillOnExit,
};
use windows_sys::Win32::System::Threading::{
    OpenProcess, PROCESS_ALL_ACCESS, PROCESS_SYNCHRONIZE, ResumeThread, SuspendThread,
    WaitForSingleObject,
};
use yog_symbols::FrameRegs;

use crate::process::arch::TRAP_FLAG;
use crate::process::target::{Event, Module, OsError, Regs, Target, Tid};

mod events;
mod sys;

/// What `SuspendThread` returns when it fails.
const SUSPEND_FAILED: u32 = u32::MAX;

/// Processes an [`interrupt`] was sent to and hasn't arrived at yet.
static INTERRUPTS: Mutex<Vec<u32>> = Mutex::new(Vec::new());

/// Asks the debugger of process `pid` to stop and report
/// [`Event::Interrupted`] from its blocking wait: Windows breaks into the
/// process on a thread of its own. Callable from any thread.
pub fn interrupt(pid: u32) {
    INTERRUPTS.locked().push(pid);
    // SAFETY: plain calls on a handle opened here and closed right after.
    unsafe {
        let process = OpenProcess(PROCESS_ALL_ACCESS, 0, pid);
        if !process.is_null() {
            DebugBreakProcess(process);
            CloseHandle(process);
        }
    }
}

/// Whether process `pid` has ended (or isn't there at all).
pub fn process_ended(pid: u32) -> bool {
    // SAFETY: a handle opened here and closed right after; a zero timeout
    // only polls.
    unsafe {
        let process = OpenProcess(PROCESS_SYNCHRONIZE, 0, pid);
        if process.is_null() {
            return true;
        }
        let ended = WaitForSingleObject(process, 0) == WAIT_OBJECT_0;
        CloseHandle(process);
        ended
    }
}

/// Whether an [`interrupt`] of `pid` is waiting: taken if so.
fn take_interrupt(pid: u32) -> bool {
    let mut pending = INTERRUPTS.locked();
    let found = pending.iter().position(|&p| p == pid);
    found.map(|i| pending.swap_remove(i)).is_some()
}

pub struct Process {
    pid: u32,
    process: HANDLE,
    /// The first thread: it stands for the process.
    leader: Option<Tid>,
    threads: HashMap<Tid, HANDLE>,
    /// The event the process waits on, and how it'll be continued.
    frozen: Option<(Tid, NTSTATUS)>,
    /// Events of other threads, kept while waiting on one.
    queue: VecDeque<Event>,
    /// Threads we hold stopped (suspended).
    held: HashSet<Tid>,
    /// Threads we single-stepped: their next single-step exception is ours.
    stepping: HashSet<Tid>,
    /// Addresses of our breakpoints (ever): a breakpoint there is ours.
    claimed: HashSet<u64>,
    modules: Vec<Module>,
    /// Names threads were given through the debugger's exception.
    names: HashMap<Tid, String>,
    /// Past the breakpoint Windows raises on attaching.
    attached: bool,
}

// SAFETY: the handles are plain kernel handles, usable from any thread;
// the debugger drives them from the one thread it attached on.
unsafe impl Send for Process {}

impl Process {
    fn thread(&self, tid: Tid) -> Result<HANDLE, OsError> {
        self.threads.get(&tid).copied().ok_or(OsError {
            what: "finding a thread",
            message: format!("thread {tid} isn't there"),
            gone: true,
            denied: false,
        })
    }

    /// Whether the process has ended (killed, say) — before its exit event
    /// arrives, its threads already refuse every request.
    fn ended(&self) -> bool {
        // SAFETY: a live process handle; a zero timeout only polls.
        !self.process.is_null() && unsafe { WaitForSingleObject(self.process, 0) } == WAIT_OBJECT_0
    }

    /// `e`, marked as a thread gone when the process has ended.
    fn or_gone(&self, mut e: OsError) -> OsError {
        e.gone |= self.ended();
        e
    }

    /// Suspends `tid`: it stays stopped while the process runs on.
    fn hold(&mut self, tid: Tid) {
        if let Ok(thread) = self.thread(tid)
            && !self.held.contains(&tid)
            // SAFETY: a live thread handle.
            && unsafe { SuspendThread(thread) } != SUSPEND_FAILED
        {
            self.held.insert(tid);
        }
    }

    fn forget(&mut self, tid: Tid) {
        self.threads.remove(&tid);
        self.held.remove(&tid);
        self.stepping.remove(&tid);
    }
}

impl Target for Process {
    fn attach(pid: u32) -> Result<(Self, Vec<Tid>), OsError> {
        // SAFETY: plain calls; the process isn't ours to kill when we go.
        if unsafe { DebugActiveProcess(pid) } == 0 {
            return Err(sys::last_error("attaching"));
        }
        // SAFETY: a plain call.
        unsafe { DebugSetProcessKillOnExit(0) };
        let mut process = Process {
            pid,
            process: std::ptr::null_mut(),
            leader: None,
            threads: HashMap::new(),
            frozen: None,
            queue: VecDeque::new(),
            held: HashSet::new(),
            stepping: HashSet::new(),
            claimed: HashSet::new(),
            modules: Vec::new(),
            names: HashMap::new(),
            attached: false,
        };
        // The process's threads and modules come as events, then Windows'
        // own breakpoint: attached.
        while !process.attached {
            process.pump()?;
        }
        process.release()?;
        let leader = process.leader.unwrap_or_default();
        let mut tids: Vec<Tid> = process.threads.keys().copied().collect();
        tids.sort_by_key(|&t| (t != leader, t));
        Ok((process, tids))
    }

    fn pid(&self) -> u32 {
        self.pid
    }

    fn leader(&self) -> Tid {
        self.leader.unwrap_or_default()
    }

    fn thread_name(&self, tid: Tid) -> String {
        if let Some(name) = self.names.get(&tid) {
            return name.clone();
        }
        self.threads
            .get(&tid)
            .map(|&t| sys::description(t))
            .unwrap_or_default()
    }

    fn stop(&mut self, tid: Tid) -> Result<(), OsError> {
        let thread = self.thread(tid)?;
        self.hold(tid);
        // Reading its context waits until it really is suspended.
        sys::context(thread)?;
        self.queue.push_back(Event::Stopped(tid));
        Ok(())
    }

    fn resume(&mut self, tid: Tid) -> Result<(), OsError> {
        event_log!("resume {tid}");
        let thread = self.thread(tid)?;
        if self.held.remove(&tid) {
            // SAFETY: a live thread handle we suspended.
            unsafe { ResumeThread(thread) };
        }
        Ok(())
    }

    fn step(&mut self, tid: Tid) -> Result<(), OsError> {
        event_log!("step {tid}");
        let thread = self.thread(tid)?;
        let mut ctx = sys::context(thread).map_err(|e| self.or_gone(e))?;
        ctx.EFlags |= TRAP_FLAG as u32;
        sys::set_context(thread, &ctx).map_err(|e| self.or_gone(e))?;
        self.stepping.insert(tid);
        self.resume(tid)
    }

    fn wait(&mut self, only: Option<Tid>) -> Result<Event, OsError> {
        event_log!("wait {only:?}, queued {:?}", self.queue);
        loop {
            if let Some(i) = self.queue.iter().position(|e| e.answers(only))
                && let Some(event) = self.queue.remove(i)
            {
                return Ok(event);
            }
            if let Some(event) = self.pump()? {
                event_log!("event {event:?}");
                if event.answers(only) {
                    return Ok(event);
                }
                self.queue.push_back(event);
            }
        }
    }

    fn regs(&self, tid: Tid) -> Result<Regs, OsError> {
        let c = sys::context(self.thread(tid)?).map_err(|e| self.or_gone(e))?;
        Ok(Regs {
            frame: FrameRegs::x86_64([
                c.Rax, c.Rdx, c.Rcx, c.Rbx, c.Rsi, c.Rdi, c.Rbp, c.Rsp, c.R8, c.R9, c.R10, c.R11,
                c.R12, c.R13, c.R14, c.R15, c.Rip,
            ]),
            named: vec![
                ("rip", c.Rip),
                ("rsp", c.Rsp),
                ("rbp", c.Rbp),
                ("rax", c.Rax),
                ("rbx", c.Rbx),
                ("rcx", c.Rcx),
                ("rdx", c.Rdx),
                ("rsi", c.Rsi),
                ("rdi", c.Rdi),
                ("r8", c.R8),
                ("r9", c.R9),
                ("r10", c.R10),
                ("r11", c.R11),
                ("r12", c.R12),
                ("r13", c.R13),
                ("r14", c.R14),
                ("r15", c.R15),
                ("eflags", u64::from(c.EFlags)),
            ],
        })
    }

    fn set_pc(&mut self, tid: Tid, pc: u64) -> Result<(), OsError> {
        let thread = self.thread(tid)?;
        let mut ctx = sys::context(thread).map_err(|e| self.or_gone(e))?;
        ctx.Rip = pc;
        sys::set_context(thread, &ctx).map_err(|e| self.or_gone(e))
    }

    fn read(&self, addr: u64, buf: &mut [u8]) -> bool {
        sys::read(self.process, addr, buf)
    }

    fn write(&mut self, addr: u64, bytes: &[u8]) -> Result<(), OsError> {
        sys::write(self.process, addr, bytes).map_err(|e| self.or_gone(e))
    }

    fn claim(&mut self, addr: u64) {
        self.claimed.insert(addr);
    }

    fn modules(&self) -> Vec<Module> {
        self.modules.clone()
    }

    fn detach(&mut self, stopped: &[Tid]) {
        for &tid in stopped {
            let _ = self.resume(tid);
        }
        for tid in self.held.clone() {
            let _ = self.resume(tid);
        }
        let _ = self.release();
        // SAFETY: a plain call; the process runs on, no longer debugged.
        unsafe { DebugActiveProcessStop(self.pid) };
        for (_, thread) in self.threads.drain() {
            // SAFETY: handles the debug events gave us.
            unsafe { CloseHandle(thread) };
        }
    }
}
