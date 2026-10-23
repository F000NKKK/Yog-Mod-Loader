//! The process's threads: stepping one, stopping and resuming them, the
//! run loop and a breakpoint hit reported.

use crate::process::arch::TRAP_ADVANCE;
use crate::process::target::{Event, Target, Tid};

use super::{DebugError, Debugger, Step, StopReason, vanished};

impl Debugger {
    /// One instruction on `tid`, absorbing stops that aren't the step
    /// itself (a stale stop request, a thread it spawned): the step is retried.
    pub(super) fn step_thread(&mut self, tid: Tid) -> Result<Step, DebugError> {
        loop {
            match self.process.step(tid) {
                Ok(()) => {}
                // Killed while stopped: its end is reported later, to
                // whatever runs next.
                Err(e) if e.gone => {
                    self.forget_thread(tid);
                    return Ok(Step::Vanished);
                }
                Err(e) => return Err(e.into()),
            }
            self.mark(tid, false);
            if let Some(step) = self.await_step(tid)? {
                return Ok(step);
            }
        }
    }

    /// Waits out a step of `tid`: what became of it, or `None` when it
    /// stopped for something else and is to be stepped again.
    fn await_step(&mut self, tid: Tid) -> Result<Option<Step>, DebugError> {
        loop {
            match self.process.wait(Some(tid))? {
                Event::Trap(_) => {
                    self.mark(tid, true);
                    return Ok(Some(Step::Stopped(StopReason::Step)));
                }
                Event::Stopped(_) | Event::Interrupted(Some(_)) => {
                    self.mark(tid, true);
                    return Ok(None);
                }
                Event::Spawned { child, parent } => {
                    self.threads.entry(child).or_insert(false);
                    if parent == Some(tid) {
                        self.mark(tid, true);
                        return Ok(None);
                    }
                }
                Event::Interrupted(None) => {}
                Event::Gone(t) => {
                    self.forget_thread(t);
                    if self.threads.is_empty() {
                        return Ok(Some(Step::Stopped(StopReason::Exited(0))));
                    }
                    return Ok(Some(Step::Vanished));
                }
                Event::Vanished(_) => return Ok(Some(Step::Vanished)),
                Event::Exited(code) => return Ok(Some(self.ended(StopReason::Exited(code)))),
                Event::Killed(sig) => return Ok(Some(self.ended(StopReason::Killed(sig)))),
            }
        }
    }

    /// The process ended: nothing is left to follow.
    fn ended(&mut self, reason: StopReason) -> Step {
        self.threads.clear();
        Step::Stopped(reason)
    }

    /// Stops following `tid`; if it was current, another stopped thread
    /// (else the leader) takes over.
    pub(super) fn forget_thread(&mut self, tid: Tid) {
        // A `pending_rearm` naming it stays: the breakpoint still needs its
        // instruction back (see `step_over_pending_breakpoint`).
        self.threads.remove(&tid);
        if self.current == tid {
            let leader = self.process.leader();
            self.current = self
                .threads
                .iter()
                .filter(|&(_, &stopped)| stopped)
                .map(|(&t, _)| t)
                .min_by_key(|&t| (t != leader, t))
                .unwrap_or(leader);
        }
    }

    pub(super) fn mark(&mut self, tid: Tid, stopped: bool) {
        self.threads.insert(tid, stopped);
    }

    pub(super) fn resume_all(&mut self) {
        let stopped: Vec<Tid> = self
            .threads
            .iter()
            .filter(|&(_, &s)| s)
            .map(|(&t, _)| t)
            .collect();
        for tid in stopped {
            // A thread that vanished meanwhile reports its end later.
            let _ = self.process.resume(tid);
            self.mark(tid, false);
        }
    }

    /// Brings the running threads `want` selects to a stop (threads born
    /// meanwhile that it selects too). A thread that had already trapped on
    /// a breakpoint is put back onto it, so it simply traps again (and gets
    /// reported) once resumed.
    pub(super) fn stop_threads(&mut self, want: impl Fn(Tid) -> bool) -> Result<(), DebugError> {
        let mut waiting: Vec<Tid> = self
            .threads
            .iter()
            .filter(|&(&tid, &stopped)| !stopped && want(tid))
            .map(|(&tid, _)| tid)
            .collect();
        for &tid in &waiting {
            let _ = self.process.stop(tid);
        }
        while let Some(tid) = waiting.pop() {
            let event = match self.process.wait(Some(tid)) {
                Ok(event) => event,
                // Already reaped / not ours any more.
                Err(_) => {
                    self.threads.remove(&tid);
                    continue;
                }
            };
            match event {
                Event::Gone(_) | Event::Exited(_) | Event::Killed(_) => {
                    self.threads.remove(&tid);
                }
                // The process is exiting: nothing to stop.
                Event::Vanished(_) => {}
                Event::Trap(_) => match self.rewind_if_trapped(tid) {
                    Ok(_) => self.mark(tid, true),
                    // Killed meanwhile; its end is reported later.
                    Err(e) if vanished(&e) => self.mark(tid, false),
                    Err(e) => return Err(e),
                },
                Event::Stopped(_) | Event::Interrupted(Some(_)) => self.mark(tid, true),
                Event::Spawned { child, parent } => {
                    let known = self.threads.insert(child, false).is_some();
                    if !known && want(child) {
                        let _ = self.process.stop(child);
                        waiting.push(child);
                    }
                    if parent == Some(tid) {
                        self.mark(tid, true);
                    } else {
                        waiting.insert(0, tid);
                    }
                }
                Event::Interrupted(None) => waiting.insert(0, tid),
            }
        }
        Ok(())
    }

    /// If `tid` just executed one of our breakpoints, puts its pc back on
    /// it. Returns the breakpoint's address if so.
    pub(super) fn rewind_if_trapped(&mut self, tid: Tid) -> Result<Option<u64>, DebugError> {
        let hit = self.process.regs(tid)?.pc().wrapping_sub(TRAP_ADVANCE);
        if !self.ever_armed.contains(&hit) {
            return Ok(None);
        }
        if TRAP_ADVANCE != 0 {
            self.process.set_pc(tid, hit)?;
        }
        Ok(Some(hit))
    }

    /// The run loop while every (known) thread is running: follows threads
    /// coming and going, and returns only for a breakpoint, an interrupt,
    /// or the end of the process.
    pub(super) fn run_until_stop(&mut self) -> Result<StopReason, DebugError> {
        loop {
            match self.process.wait(None)? {
                Event::Exited(code) => {
                    self.threads.clear();
                    return Ok(StopReason::Exited(code));
                }
                Event::Killed(sig) => {
                    self.threads.clear();
                    return Ok(StopReason::Killed(sig));
                }
                Event::Gone(tid) => self.forget_thread(tid),
                Event::Vanished(_) => {}
                Event::Spawned { child, parent } => {
                    self.threads.entry(child).or_insert(false);
                    if let Some(parent) = parent {
                        let _ = self.process.resume(parent);
                    }
                }
                // A new thread's first stop, or a stale stop request: run on.
                Event::Stopped(tid) => {
                    self.mark(tid, false);
                    let _ = self.process.resume(tid);
                }
                Event::Trap(tid) => {
                    if let Some(reason) = self.trapped(tid)? {
                        return Ok(reason);
                    }
                }
                Event::Interrupted(tid) => {
                    if let Some(tid) = tid {
                        self.mark(tid, true);
                    }
                    return Ok(StopReason::Interrupted);
                }
            }
        }
    }

    /// `tid` trapped while running: a breakpoint hit to report, or a trap
    /// that's no longer anything (a breakpoint cleared before it surfaced),
    /// after which it runs on.
    fn trapped(&mut self, tid: Tid) -> Result<Option<StopReason>, DebugError> {
        self.mark(tid, true);
        let hit = match self.rewind_if_trapped(tid) {
            Ok(hit) => hit,
            // Killed right after trapping (the process exiting): its end comes next.
            Err(e) if vanished(&e) => {
                self.mark(tid, false);
                return Ok(None);
            }
            Err(e) => return Err(e),
        };
        match hit {
            Some(addr) if self.breakpoints.contains_key(&addr) => {
                match self.report_breakpoint(tid, addr) {
                    Err(e) if vanished(&e) => {
                        self.mark(tid, false);
                        Ok(None)
                    }
                    reported => reported.map(Some),
                }
            }
            _ => {
                self.mark(tid, false);
                let _ = self.process.resume(tid);
                Ok(None)
            }
        }
    }

    pub(super) fn report_breakpoint(
        &mut self,
        tid: Tid,
        addr: u64,
    ) -> Result<StopReason, DebugError> {
        // Restore the real instruction, but do NOT step past it yet — the
        // thread stays genuinely paused at `addr` until the next
        // continue/step, so backtraces/inspection see the real location.
        let original = self.breakpoints[&addr].clone();
        self.process.write(addr, &original)?;
        self.current = tid;
        self.pending_rearm = Some((addr, tid));
        Ok(StopReason::Breakpoint(addr))
    }
}
