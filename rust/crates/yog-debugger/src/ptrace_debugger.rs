//! Real native breakpoint debugging via Linux `ptrace` — classic INT3
//! (`0xCC`) byte-patch breakpoints, `waitpid`-driven stop handling, and a
//! frame-pointer backtrace. Address resolution against source is delegated
//! entirely to `yog-symbols`; this module only knows about raw addresses in
//! the attached process.

use std::collections::HashMap;

use iced_x86::{Decoder, DecoderOptions, Mnemonic};
use nix::sys::ptrace;
use nix::sys::signal::Signal;
use nix::sys::wait::{waitpid, WaitStatus};
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

impl From<nix::Error> for DebugError {
    fn from(e: nix::Error) -> Self {
        DebugError::Ptrace(e)
    }
}

/// Why the tracee stopped running, after a `cont`/`single_step`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StopReason {
    /// Hit one of our own breakpoints — already rewound/re-armed, `rip` is
    /// at the breakpoint address.
    Breakpoint(u64),
    /// Stopped for some other signal (a real crash, or a signal the tracee
    /// itself would normally handle).
    Signal(Signal),
    Exited(i32),
    Killed(Signal),
}

/// One live `ptrace` session against a running process (or thread group
/// leader — this MVP does not yet handle multi-threaded breakpoint
/// propagation across sibling threads).
pub struct Debugger {
    pid: Pid,
    /// Runtime address -> the original byte our `0xCC` replaced.
    breakpoints: HashMap<u64, u8>,
    /// Set right after a breakpoint hit: the address whose original byte is
    /// currently (temporarily) restored so the tracee sits paused exactly
    /// at the breakpoint for inspection. The next `continue_`/`single_step`
    /// must step over it and re-arm the `0xCC` before doing anything else,
    /// or the same breakpoint would never fire again.
    pending_rearm: Option<u64>,
}

impl Debugger {
    pub fn attach(pid: i32) -> Result<Self, DebugError> {
        let pid = Pid::from_raw(pid);
        ptrace::attach(pid)?;
        waitpid(pid, None)?;
        Ok(Debugger { pid, breakpoints: HashMap::new(), pending_rearm: None })
    }

    pub fn pid(&self) -> Pid {
        self.pid
    }

    pub fn detach(&mut self) -> Result<(), DebugError> {
        for addr in self.breakpoints.keys().copied().collect::<Vec<_>>() {
            let _ = self.clear_breakpoint(addr);
        }
        ptrace::detach(self.pid, None)?;
        Ok(())
    }

    fn peek(&self, addr: u64) -> Result<i64, DebugError> {
        Ok(ptrace::read(self.pid, addr as ptrace::AddressType)?)
    }

    fn poke(&self, addr: u64, data: i64) -> Result<(), DebugError> {
        ptrace::write(self.pid, addr as ptrace::AddressType, data)?;
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

    pub fn continue_(&mut self) -> Result<StopReason, DebugError> {
        self.step_over_pending_breakpoint()?;
        ptrace::cont(self.pid, None)?;
        self.wait_and_resolve()
    }

    pub fn single_step(&mut self) -> Result<StopReason, DebugError> {
        self.step_over_pending_breakpoint()?;
        ptrace::step(self.pid, None)?;
        self.wait_and_resolve()
    }

    /// If we're paused right at a breakpoint (its original byte temporarily
    /// restored for inspection), execute that one real instruction and
    /// re-arm the `0xCC` before doing anything else the caller asked for —
    /// otherwise the same address could never trap again.
    fn step_over_pending_breakpoint(&mut self) -> Result<(), DebugError> {
        let Some(addr) = self.pending_rearm.take() else { return Ok(()) };
        ptrace::step(self.pid, None)?;
        match waitpid(self.pid, None)? {
            WaitStatus::Exited(..) | WaitStatus::Signaled(..) => return Ok(()),
            _ => {}
        }
        if self.breakpoints.contains_key(&addr) {
            let word = self.peek(addr)?;
            self.poke(addr, (word & !0xffi64) | 0xCC)?;
        }
        Ok(())
    }

    fn wait_and_resolve(&mut self) -> Result<StopReason, DebugError> {
        match waitpid(self.pid, None)? {
            WaitStatus::Exited(_, code) => Ok(StopReason::Exited(code)),
            WaitStatus::Signaled(_, sig, _) => Ok(StopReason::Killed(sig)),
            WaitStatus::Stopped(_, Signal::SIGTRAP) => {
                let mut regs = ptrace::getregs(self.pid)?;
                let hit_addr = regs.rip.wrapping_sub(1);
                if self.breakpoints.contains_key(&hit_addr) {
                    // Rewind past the INT3 and restore the real instruction
                    // byte, but do NOT step past it yet — the tracee stays
                    // genuinely paused at `hit_addr` until the next
                    // continue/step, so backtraces/inspection see the real
                    // stop location.
                    regs.rip = hit_addr;
                    ptrace::setregs(self.pid, regs)?;
                    let original_byte = *self.breakpoints.get(&hit_addr).expect("checked above");
                    let word = self.peek(hit_addr)?;
                    self.poke(hit_addr, (word & !0xffi64) | original_byte as i64)?;
                    self.pending_rearm = Some(hit_addr);
                    Ok(StopReason::Breakpoint(hit_addr))
                } else {
                    Ok(StopReason::Signal(Signal::SIGTRAP))
                }
            }
            WaitStatus::Stopped(_, sig) => Ok(StopReason::Signal(sig)),
            other => Err(DebugError::UnexpectedStop(format!("{other:?}"))),
        }
    }

    pub fn registers(&self) -> Result<nix::libc::user_regs_struct, DebugError> {
        Ok(ptrace::getregs(self.pid)?)
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
            let Ok(return_addr) = self.peek(bp.wrapping_add(8)) else { break };
            if return_addr == 0 {
                break;
            }
            frames.push(return_addr as u64);
            bp = saved_bp as u64;
        }

        Ok(frames)
    }
}
