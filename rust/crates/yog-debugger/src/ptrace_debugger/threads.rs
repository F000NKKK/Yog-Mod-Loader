//! The tracee's threads: waiting on them, stepping one, stopping and
//! resuming them all, a stop reported.

use std::collections::HashSet;

use nix::sys::ptrace;
use nix::sys::signal::Signal;
use nix::sys::wait::{WaitPidFlag, WaitStatus, waitpid};
use nix::unistd::Pid;

use super::{
    DebugError, Debugger, Step, StopReason, process_exiting, vanished, wait_flags, with_first_byte,
};

impl Debugger {
    /// `waitpid` on one thread — except that a leader whose process is
    /// exiting never reports while other threads are still unreaped (which
    /// only waiting on *them* does), so that returns `None` instead of
    /// blocking forever.
    pub(super) fn wait_thread(&self, tid: Pid) -> nix::Result<Option<WaitStatus>> {
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
    pub(super) fn step_thread(&mut self, tid: Pid) -> Result<Step, DebugError> {
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
                    return Ok(self.thread_gone(tid, StopReason::Exited(code)));
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
    pub(super) fn thread_gone(&mut self, tid: Pid, reason: StopReason) -> Step {
        self.forget_thread(tid);
        if tid == self.pid || self.threads.is_empty() {
            Step::Stopped(reason)
        } else {
            Step::Vanished
        }
    }

    /// Stops tracking `tid`; if it was current, another stopped thread
    /// (else the leader) takes over.
    pub(super) fn forget_thread(&mut self, tid: Pid) {
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

    pub(super) fn mark(&mut self, tid: Pid, stopped: bool) {
        self.threads.entry(tid).or_default().stopped = stopped;
    }

    /// `tid` reported a clone event: track the new thread (it starts in a
    /// ptrace-stop of its own, which `run_until_stop` resumes). If `tid`
    /// vanished before saying which thread it made, the child is still
    /// picked up from its own first stop.
    pub(super) fn adopt_clone(&mut self, tid: Pid) -> Option<Pid> {
        let child = Pid::from_raw(ptrace::getevent(tid).ok()? as i32);
        self.threads.entry(child).or_default();
        Some(child)
    }

    pub(super) fn resume_all(&mut self) -> Result<(), DebugError> {
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
    pub(super) fn stop_threads(&mut self, want: impl Fn(Pid) -> bool) -> Result<(), DebugError> {
        let running: Vec<Pid> = self
            .threads
            .iter()
            .filter(|&(&tid, t)| !t.stopped && want(tid))
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
                        if let Some(child) = self.adopt_clone(tid)
                            && !known.contains(&child)
                        {
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
    pub(super) fn rewind_if_trapped(&mut self, tid: Pid) -> Result<Option<u64>, DebugError> {
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
    pub(super) fn run_until_stop(&mut self) -> Result<StopReason, DebugError> {
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

    pub(super) fn report_breakpoint(
        &mut self,
        tid: Pid,
        addr: u64,
    ) -> Result<StopReason, DebugError> {
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
}
