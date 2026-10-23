//! Source-level debugging: where the thread is in the source, steps by
//! line (over, into, out — Just My Code), registers and backtraces.

use yog_symbols::{FrameRegs, SourceLocation, SymbolTable, TargetMemory};

use super::{
    CALLER_SEARCH_DEPTH, DebugError, Debugger, Landed, LineKey, MAX_SOURCE_STEPS, Position,
    StepKind, StopReason, is_terminal,
};
use crate::process::arch::{LONGEST, call_length};
use crate::process::target::{Regs, Target};
use crate::{is_library_source, user_location};

/// Bytes of a pointer in the debugged process.
const WORD: usize = 8;

impl Debugger {
    /// The current thread's registers.
    pub fn registers(&self) -> Result<Regs, DebugError> {
        Ok(self.process.regs(self.current)?)
    }

    /// The process's memory, for reading values (`yog-symbols`' locals).
    pub fn memory(&self) -> impl TargetMemory + '_ {
        ProcessMemory(self)
    }

    /// The source location the pc currently maps to, or `None` if the current
    /// instruction is in code with no line info (a library, or a
    /// compiler-generated stub). `base` is the mod native's load address —
    /// see `crate::maps::find_module_base_by_prefix`.
    pub fn source_location(&self, symbols: &SymbolTable, base: u64) -> Option<SourceLocation> {
        let offset = self.registers().ok()?.pc().checked_sub(base)?;
        symbols.resolve_addr(offset)
    }

    /// A comparable (file, line) key for "are we still on the same source
    /// line" checks during stepping: the innermost *user* line at the pc
    /// (see [`user_location`]) — `None` in library code or code without
    /// lines, which stepping never stops in.
    pub(super) fn line_key(&self, symbols: &SymbolTable, base: u64) -> Option<LineKey> {
        match self.position(symbols, base) {
            Position::User(key) => Some(key),
            Position::NoLine | Position::Foreign => None,
        }
    }

    /// What kind of code the pc is in, for stepping.
    pub(super) fn position(&self, symbols: &SymbolTable, base: u64) -> Position {
        let Some(offset) = self.registers().ok().and_then(|r| r.pc().checked_sub(base)) else {
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

    /// Reads `buf.len()` bytes of code at `addr`, the original instructions
    /// put back for any breakpoints armed within the window — so a
    /// breakpoint never corrupts the decode of the instruction looked at.
    pub(super) fn read_code(&self, addr: u64, buf: &mut [u8]) -> Result<(), DebugError> {
        if !self.process.read(addr, buf) {
            return Err(DebugError::UnexpectedStop(format!(
                "{addr:#x} isn't readable"
            )));
        }
        let end = addr + buf.len() as u64;
        for (&bp, original) in &self.breakpoints {
            for (i, &byte) in original.iter().enumerate() {
                let at = bp + i as u64;
                if (addr..end).contains(&at) {
                    buf[(at - addr) as usize] = byte;
                }
            }
        }
        Ok(())
    }

    /// The instruction at `pc`: `(is_call, length)`. Falls back to
    /// `(false, 1)` on unreadable memory — a caller then just single-steps.
    pub(super) fn decode_at(&self, pc: u64) -> (bool, u64) {
        let mut buf = [0u8; LONGEST];
        if self.read_code(pc, &mut buf).is_err() {
            return (false, 1);
        }
        match call_length(&buf, pc) {
            Some(len) => (true, len),
            None => (false, 1),
        }
    }

    /// Runs until `addr` is reached (via a temporary breakpoint) — used to
    /// step over a call by returning to the instruction after it. Stops
    /// early and reports it if a *user* breakpoint is hit along the way.
    /// The temporary breakpoint only counts for the stepping thread back in
    /// the stepping frame: other threads running the same code, or a
    /// recursive call reaching it deeper in the stack, just pass through.
    pub(super) fn run_to(&mut self, addr: u64) -> Result<StopReason, DebugError> {
        let was_user_bp = self.breakpoints.contains_key(&addr);
        if !was_user_bp {
            self.set_breakpoint(addr)?;
        }
        let origin = self.current;
        let origin_sp = self.registers()?.sp();
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
                    let here = self.registers().map(|r| r.sp()).unwrap_or(0);
                    if self.current == origin && here >= origin_sp {
                        break Ok(StopReason::Step);
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
    /// the process exits. `symbols`/`base` resolve the pc to a source line.
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
        for _ in 0..MAX_SOURCE_STEPS {
            if self.pause_requested() {
                return Ok(StopReason::Interrupted);
            }
            let reason = self.step_instruction(kind, symbols, base)?;
            // Ended, a breakpoint, a pause, another signal: the step stops.
            if reason != StopReason::Step {
                return Ok(reason);
            }
            match self.landed(symbols, base)? {
                Landed::Line(cur) => {
                    // Only where a statement begins: mid-statement rows are the
                    // compiler interleaving lines, not the code moving on.
                    let begins = self
                        .registers()
                        .is_ok_and(|r| symbols.is_statement_start(r.pc().wrapping_sub(base)));
                    if cur != start && begins {
                        return Ok(StopReason::Step);
                    }
                }
                Landed::Within => {}
                Landed::Stop(reason) => return Ok(reason),
            }
        }
        Ok(StopReason::Step)
    }

    /// One instruction of a source step: a call stepped over (`Over`) or
    /// into — and straight back out of library code (VS's Just My Code).
    pub(super) fn step_instruction(
        &mut self,
        kind: StepKind,
        symbols: &SymbolTable,
        base: u64,
    ) -> Result<StopReason, DebugError> {
        let pc = self.registers()?.pc();
        let (is_call, len) = self.decode_at(pc);
        if !is_call {
            return self.single_step();
        }
        if kind == StepKind::Over {
            return self.run_to(pc + len);
        }
        let stepped = self.single_step()?;
        // Ended, or the thread exited and the process ran on.
        if stepped != StopReason::Step || self.line_key(symbols, base).is_some() {
            return Ok(stepped);
        }
        // Into library code (std, a dependency, anything without lines):
        // run back to the instruction after the call.
        let back = self.run_to(pc + len)?;
        if is_terminal(&back) || matches!(back, StopReason::Breakpoint(_) | StopReason::Interrupted)
        {
            return Ok(back);
        }
        Ok(StopReason::Step)
    }

    /// Where a step landed. Stepped off the end of user code — into the
    /// JVM that called the mod, into the `std` function that called a
    /// closure, a tail jump into a library — it goes back to the user
    /// caller (crawling through that an instruction at a time looked hung).
    pub(super) fn landed(
        &mut self,
        symbols: &SymbolTable,
        base: u64,
    ) -> Result<Landed, DebugError> {
        Ok(match self.position(symbols, base) {
            Position::User(key) => Landed::Line(key),
            // A compiler-generated row (line 0) of user code: keep going.
            Position::NoLine => Landed::Within,
            Position::Foreign => {
                // Back in the user caller (mid-statement, right after its
                // call), where stepping goes on as after any return.
                let back = self.leave_foreign_code(symbols, base)?;
                match self.line_key(symbols, base) {
                    Some(key) if back == StopReason::Step => Landed::Line(key),
                    _ => Landed::Stop(back),
                }
            }
        })
    }

    /// Shift+F11: runs to the return address of the current frame (found by
    /// CFI), then on out of any library code to the nearest user caller.
    pub(super) fn step_out(
        &mut self,
        symbols: &SymbolTable,
        base: u64,
    ) -> Result<StopReason, DebugError> {
        let frames = self.backtrace_with(symbols, base, 2)?;
        let Some(caller) = frames.get(1) else {
            return self.run_on();
        };
        let back = self.run_to(caller.pc().wrapping_add(1))?;
        if back != StopReason::Step {
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
    pub(super) fn leave_foreign_code(
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
        Ok(self.registers()?.frame)
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
        Ok(symbols.backtrace(&regs, base, &ProcessMemory(self), max))
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
        let mut frames = vec![regs.pc()];
        let mut fp = regs.fp();
        let mem = ProcessMemory(self);
        while fp != 0 && frames.len() < max_frames {
            let (Some(saved_fp), Some(ret)) =
                (word(&mem, fp), word(&mem, fp.wrapping_add(WORD as u64)))
            else {
                break;
            };
            if ret == 0 {
                break;
            }
            frames.push(ret);
            fp = saved_fp;
        }
        Ok(frames)
    }
}

/// The word at `addr`.
fn word(mem: &dyn TargetMemory, addr: u64) -> Option<u64> {
    let mut buf = [0u8; WORD];
    mem.read(addr, &mut buf).then(|| u64::from_le_bytes(buf))
}

/// The debugged process's memory.
struct ProcessMemory<'a>(&'a Debugger);

impl TargetMemory for ProcessMemory<'_> {
    fn read(&self, addr: u64, buf: &mut [u8]) -> bool {
        self.0.process.read(addr, buf)
    }
}
