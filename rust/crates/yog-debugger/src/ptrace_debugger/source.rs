//! Source-level debugging: where the thread is in the source, steps by
//! line (over, into, out — Just My Code), registers and backtraces.

use nix::sys::ptrace;
use nix::sys::signal::Signal;
use yog_symbols::{FrameRegs, SourceLocation, SymbolTable, TargetMemory};

use super::{
    CALLER_SEARCH_DEPTH, DebugError, Debugger, Landed, LineKey, MAX_SOURCE_STEPS, Position,
    StepKind, StopReason, is_terminal,
};
use crate::{is_library_source, user_location};
use iced_x86::{Decoder, DecoderOptions, Mnemonic};

impl Debugger {
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
    pub(super) fn line_key(&self, symbols: &SymbolTable, base: u64) -> Option<LineKey> {
        match self.position(symbols, base) {
            Position::User(key) => Some(key),
            Position::NoLine | Position::Foreign => None,
        }
    }

    /// What kind of code `rip` is in, for stepping.
    pub(super) fn position(&self, symbols: &SymbolTable, base: u64) -> Position {
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
    pub(super) fn read_code(&self, addr: u64, buf: &mut [u8]) -> Result<(), DebugError> {
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
    pub(super) fn decode_at(&self, rip: u64) -> (bool, u64) {
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
    pub(super) fn run_to(&mut self, addr: u64) -> Result<StopReason, DebugError> {
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
        for _ in 0..MAX_SOURCE_STEPS {
            if self.pause_requested() {
                return Ok(StopReason::Interrupted);
            }
            let reason = self.step_instruction(kind, symbols, base)?;
            // Ended, a breakpoint, a pause, another signal: the step stops.
            if reason != StopReason::Signal(Signal::SIGTRAP) {
                return Ok(reason);
            }
            match self.landed(symbols, base)? {
                Landed::Line(cur) => {
                    // Only where a statement begins: mid-statement rows are the
                    // compiler interleaving lines, not the code moving on.
                    let begins = self
                        .registers()
                        .is_ok_and(|r| symbols.is_statement_start(r.rip.wrapping_sub(base)));
                    if cur != start && begins {
                        return Ok(StopReason::Signal(Signal::SIGTRAP));
                    }
                }
                Landed::Within => {}
                Landed::Stop(reason) => return Ok(reason),
            }
        }
        Ok(StopReason::Signal(Signal::SIGTRAP))
    }

    /// One instruction of a source step: a call stepped over (`Over`) or
    /// into — and straight back out of library code (VS's Just My Code).
    pub(super) fn step_instruction(
        &mut self,
        kind: StepKind,
        symbols: &SymbolTable,
        base: u64,
    ) -> Result<StopReason, DebugError> {
        let rip = self.registers()?.rip;
        let (is_call, len) = self.decode_at(rip);
        if !is_call {
            return self.single_step();
        }
        if kind == StepKind::Over {
            return self.run_to(rip + len);
        }
        let stepped = self.single_step()?;
        // Ended, or the thread exited and the process ran on.
        if stepped != StopReason::Signal(Signal::SIGTRAP) || self.line_key(symbols, base).is_some()
        {
            return Ok(stepped);
        }
        // Into library code (std, a dependency, anything without lines):
        // run back to the instruction after the call.
        let back = self.run_to(rip + len)?;
        if is_terminal(&back) || matches!(back, StopReason::Breakpoint(_) | StopReason::Interrupted)
        {
            return Ok(back);
        }
        Ok(StopReason::Signal(Signal::SIGTRAP))
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
                    Some(key) if back == StopReason::Signal(Signal::SIGTRAP) => Landed::Line(key),
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
