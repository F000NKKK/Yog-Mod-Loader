//! Linux: `ptrace`. Every thread is seized (`PTRACE_O_TRACECLONE`: new ones
//! too) and reported by `waitpid`; memory goes through `/proc/<pid>/mem`.
//! Signals the JVM relies on (SIGSEGV for safepoints and null checks, ...)
//! and traps that aren't ours are handed straight back to it.

use std::collections::{HashMap, HashSet};
use std::os::unix::fs::FileExt;
use std::path::PathBuf;

use nix::sys::ptrace;
use nix::sys::signal::Signal;
use nix::sys::wait::{WaitPidFlag, WaitStatus, waitpid};
use nix::unistd::Pid;

use crate::process::arch::TRAP_ADVANCE;
use crate::process::target::{Event, Module, OsError, Regs, Target, Tid};

mod regs;

/// How long a wait on an exiting process's leader sleeps between polls.
const LEADER_POLL: std::time::Duration = std::time::Duration::from_micros(200);

pub struct Process {
    pid: Pid,
    /// `/proc/<pid>/mem`, read and written whatever the threads are doing.
    mem: std::fs::File,
    /// A signal a thread got while we stopped it, delivered when it runs again.
    pending: HashMap<Tid, Signal>,
    /// Threads we single-stepped: their next SIGTRAP is the step's end.
    stepping: HashSet<Tid>,
    /// Addresses of our breakpoints (ever): a SIGTRAP just past one is ours.
    claimed: HashSet<u64>,
}

fn os(what: &'static str) -> impl Fn(nix::Error) -> OsError {
    move |e| OsError {
        what,
        message: e.to_string(),
        // ESRCH: the thread isn't in a ptrace-stop any more (killed).
        gone: e == nix::errno::Errno::ESRCH,
        denied: e == nix::errno::Errno::EPERM,
    }
}

fn pid_of(tid: Tid) -> Pid {
    Pid::from_raw(tid as i32)
}

/// `__WALL` so clone()d threads are reported; `__WNOTHREAD` so a
/// `waitpid(-1)` only reaps this thread's own tracees and never steals exit
/// statuses of processes spawned by other host threads.
fn wait_flags() -> Option<WaitPidFlag> {
    Some(WaitPidFlag::__WALL | WaitPidFlag::__WNOTHREAD)
}

fn task_ids(pid: Pid) -> Vec<Tid> {
    let Ok(dir) = std::fs::read_dir(format!("/proc/{pid}/task")) else {
        return Vec::new();
    };
    dir.filter_map(|e| e.ok()?.file_name().to_str()?.parse().ok())
        .collect()
}

/// Whether process `pid` has ended (or is ending): its leader a zombie (it
/// is one until every other thread has been reaped) or gone altogether.
pub fn process_ended(pid: u32) -> bool {
    let Ok(stat) = std::fs::read_to_string(format!("/proc/{pid}/stat")) else {
        return true;
    };
    // `pid (comm) S ...` — `comm` may itself contain spaces or parentheses.
    let state = stat
        .rsplit_once(')')
        .and_then(|(_, rest)| rest.trim_start().chars().next());
    matches!(state, Some('Z' | 'X') | None)
}

/// Asks the debugger of process `pid` to stop and report
/// [`Event::Interrupted`] from its blocking wait. Callable from any thread.
pub fn interrupt(pid: u32) {
    // `tgkill`, not `kill`: a process-directed SIGSTOP could land on any
    // thread and group-stop the whole JVM. The tracer swallows it, so it
    // never reaches the tracee.
    // SAFETY: a plain syscall taking only integer arguments.
    unsafe {
        nix::libc::syscall(nix::libc::SYS_tgkill, pid, pid, nix::libc::SIGSTOP);
    }
}

impl Process {
    fn seize(tid: Tid) -> nix::Result<()> {
        ptrace::seize(pid_of(tid), ptrace::Options::PTRACE_O_TRACECLONE)
    }

    /// A SIGTRAP on `tid`: ours (a step's end, a breakpoint of ours) or the
    /// process's own.
    fn ours(&mut self, tid: Tid) -> bool {
        if self.stepping.remove(&tid) {
            return true;
        }
        self.regs(tid)
            .is_ok_and(|r| self.claimed.contains(&r.pc().wrapping_sub(TRAP_ADVANCE)))
    }

    /// `waitpid` on one thread — except that a leader whose process is
    /// exiting never reports while other threads are still unreaped, so
    /// that returns `None` instead of blocking forever.
    fn wait_status(&self, only: Option<Tid>) -> nix::Result<Option<WaitStatus>> {
        let Some(tid) = only else {
            return waitpid(None, wait_flags()).map(Some);
        };
        if pid_of(tid) != self.pid {
            return waitpid(pid_of(tid), wait_flags()).map(Some);
        }
        loop {
            match waitpid(self.pid, wait_flags().map(|f| f | WaitPidFlag::WNOHANG))? {
                WaitStatus::StillAlive => {
                    if process_ended(self.pid()) {
                        return Ok(None);
                    }
                    std::thread::sleep(LEADER_POLL);
                }
                status => return Ok(Some(status)),
            }
        }
    }

    /// What `status` (of `tid`) means; `None` when it was the process's own
    /// business, handed back to it.
    fn event(&mut self, tid: Tid, status: WaitStatus) -> Option<Event> {
        let leader = pid_of(tid) == self.pid;
        Some(match status {
            WaitStatus::Exited(_, code) if leader => Event::Exited(code),
            WaitStatus::Signaled(_, sig, _) if leader => Event::Killed(sig as i32),
            WaitStatus::Exited(..) | WaitStatus::Signaled(..) => {
                self.pending.remove(&tid);
                Event::Gone(tid)
            }
            WaitStatus::PtraceEvent(_, _, event) if event == nix::libc::PTRACE_EVENT_CLONE => {
                let child = ptrace::getevent(pid_of(tid)).ok()? as Tid;
                Event::Spawned {
                    child,
                    parent: Some(tid),
                }
            }
            // A new thread's first stop, our `stop`'s, a group-stop.
            WaitStatus::PtraceEvent(..) => Event::Stopped(tid),
            WaitStatus::Stopped(_, Signal::SIGSTOP) => Event::Interrupted(Some(tid)),
            WaitStatus::Stopped(_, Signal::SIGTRAP) if self.ours(tid) => Event::Trap(tid),
            WaitStatus::Stopped(_, sig) => {
                // Not ours: it gets the signal back when it runs on.
                self.pending.insert(tid, sig);
                return None;
            }
            _ => return None,
        })
    }
}

impl Target for Process {
    fn attach(pid: u32) -> Result<(Self, Vec<Tid>), OsError> {
        let leader = Tid::from(pid);
        let mem = std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .open(format!("/proc/{pid}/mem"));
        Process::seize(leader).map_err(os("attach"))?;
        let mem = mem.map_err(|e| OsError {
            what: "opening its memory",
            message: e.to_string(),
            gone: false,
            denied: false,
        })?;
        let process = Process {
            pid: Pid::from_raw(pid as i32),
            mem,
            pending: HashMap::new(),
            stepping: HashSet::new(),
            claimed: HashSet::new(),
        };
        let mut threads = vec![leader];
        // Threads can start while we enumerate; ones spawned by an already-
        // seized thread are auto-attached (TRACECLONE), the rest show up on
        // the next pass.
        loop {
            let fresh: Vec<Tid> = task_ids(process.pid)
                .into_iter()
                .filter(|t| !threads.contains(t))
                .collect();
            if fresh.is_empty() {
                break;
            }
            threads.extend(fresh.into_iter().filter(|&t| Process::seize(t).is_ok()));
        }
        Ok((process, threads))
    }

    fn pid(&self) -> u32 {
        self.pid.as_raw() as u32
    }

    fn leader(&self) -> Tid {
        self.pid.as_raw() as Tid
    }

    fn thread_name(&self, tid: Tid) -> String {
        std::fs::read_to_string(format!("/proc/{}/task/{tid}/comm", self.pid))
            .map(|n| n.trim_end().to_string())
            .unwrap_or_default()
    }

    fn stop(&mut self, tid: Tid) -> Result<(), OsError> {
        ptrace::interrupt(pid_of(tid)).map_err(os("stop"))
    }

    fn resume(&mut self, tid: Tid) -> Result<(), OsError> {
        event_log!("resume {tid}");
        let signal = self.pending.remove(&tid);
        ptrace::cont(pid_of(tid), signal).map_err(os("resume"))
    }

    fn step(&mut self, tid: Tid) -> Result<(), OsError> {
        event_log!("step {tid}");
        let signal = self.pending.remove(&tid);
        self.stepping.insert(tid);
        ptrace::step(pid_of(tid), signal).map_err(os("step"))
    }

    fn wait(&mut self, only: Option<Tid>) -> Result<Event, OsError> {
        loop {
            let Some(status) = self.wait_status(only).map_err(os("wait"))? else {
                return Ok(Event::Vanished(only.unwrap_or_default()));
            };
            event_log!("status {status:?}");
            let Some(tid) = status.pid().map(|p| p.as_raw() as Tid) else {
                continue;
            };
            match self.event(tid, status) {
                Some(event) => return Ok(event),
                // The process's own signal or trap: back to it.
                None => {
                    let _ = self.resume(tid);
                }
            }
        }
    }

    fn regs(&self, tid: Tid) -> Result<Regs, OsError> {
        regs::of(pid_of(tid)).map_err(os("registers"))
    }

    fn set_pc(&mut self, tid: Tid, pc: u64) -> Result<(), OsError> {
        regs::set_pc(pid_of(tid), pc).map_err(os("setting the pc"))
    }

    fn read(&self, addr: u64, buf: &mut [u8]) -> bool {
        self.mem.read_exact_at(buf, addr).is_ok()
    }

    fn write(&mut self, addr: u64, bytes: &[u8]) -> Result<(), OsError> {
        self.mem.write_all_at(bytes, addr).map_err(|e| OsError {
            what: "writing its memory",
            message: e.to_string(),
            // Killed: its memory is torn down (a write then takes nothing)
            // before its end is reported.
            gone: e.kind() == std::io::ErrorKind::WriteZero || process_ended(self.pid()),
            denied: false,
        })
    }

    fn claim(&mut self, addr: u64) {
        self.claimed.insert(addr);
    }

    fn modules(&self) -> Vec<Module> {
        let maps = std::fs::read_to_string(format!("/proc/{}/maps", self.pid)).unwrap_or_default();
        let mut out: Vec<Module> = Vec::new();
        for line in maps.lines() {
            // start-end perms offset dev inode [pathname]
            let fields: Vec<&str> = line.split_whitespace().collect();
            let Some(path) = fields.get(5..).map(|p| p.join(" ")) else {
                continue;
            };
            let base = fields[0]
                .split('-')
                .next()
                .and_then(|h| u64::from_str_radix(h, 16).ok());
            let (Some(base), false) = (base, path.is_empty()) else {
                continue;
            };
            let path = PathBuf::from(path);
            // A file's first mapping is its lowest address: its load base.
            if !out.iter().any(|m| m.path == path) {
                out.push(Module { base, path });
            }
        }
        out
    }

    fn detach(&mut self, stopped: &[Tid]) {
        for &tid in stopped {
            let _ = ptrace::detach(pid_of(tid), self.pending.remove(&tid));
        }
    }
}
