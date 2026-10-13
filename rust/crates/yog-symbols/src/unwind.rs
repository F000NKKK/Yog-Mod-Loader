//! Stack unwinding from call frame info (`.eh_frame` / `.debug_frame`).
//!
//! The prebuilt Rust standard library (statically linked into every mod
//! native) is compiled without frame pointers, so an `rbp` chain walk loses
//! the stack at the first std frame. CFI describes every frame regardless,
//! so it's used wherever the module has an FDE; `rbp` chaining is only the
//! fallback for code without one (e.g. the JVM, which keeps frame pointers).

use gimli::{RegisterRule, UnwindSection as _};

use crate::locals::{x86_64, FrameRegs, TargetMemory, FP_TO_CFA, WORD_SIZE};
use crate::SymbolTable;

/// Registers a callee must preserve (System V x86_64) — the only ones whose
/// caller value CFI can recover besides `rsp`/`rip`.
const CALLEE_SAVED: [u16; 6] = [
    x86_64::RBX,
    x86_64::RBP,
    x86_64::R12,
    x86_64::R13,
    x86_64::R14,
    x86_64::R15,
];

impl SymbolTable {
    /// Unwinds one frame: the caller's registers given the callee's `regs`
    /// (absolute addresses, module loaded at `load_base`).
    ///
    /// The returned frame's pc is the return address minus one, so it
    /// resolves to the call instruction's line (the convention for every
    /// outer frame; pass it straight to [`SymbolTable::read_locals`]).
    /// Uses the module's CFI when it has an FDE for the pc, otherwise walks
    /// the `rbp` chain. `None` at the end of the stack or when the result
    /// can't be trusted (unreadable memory, a stack pointer that doesn't
    /// grow).
    pub fn unwind(
        &self,
        regs: &FrameRegs,
        load_base: u64,
        mem: &dyn TargetMemory,
    ) -> Option<FrameRegs> {
        let caller = match self.unwind_cfi(regs, load_base, mem) {
            Some(caller) => caller,
            None => unwind_frame_pointer(regs, mem)?,
        };
        let (ret, sp) = (caller.pc().wrapping_add(1), caller.sp()?);
        if ret <= 1 || sp <= regs.sp()? {
            return None;
        }
        Some(caller)
    }

    /// Up to `max` frames starting with `regs` itself, innermost first,
    /// unwound with [`SymbolTable::unwind`]. Frames outside this module are
    /// walked through by `rbp` until the stack reaches the module; once it
    /// has been in the module, the first frame outside it ends the walk
    /// (its caller can't be found reliably from here).
    pub fn backtrace(
        &self,
        regs: &FrameRegs,
        load_base: u64,
        mem: &dyn TargetMemory,
        max: usize,
    ) -> Vec<FrameRegs> {
        let mut frames = Vec::new();
        let mut current = regs.clone();
        let mut seen_module = false;
        while frames.len() < max {
            let inside = self.contains(current.pc().wrapping_sub(load_base));
            if !inside && seen_module {
                frames.push(current);
                break;
            }
            seen_module |= inside;
            let next = self.unwind(&current, load_base, mem);
            frames.push(current);
            match next {
                Some(next) => current = next,
                None => break,
            }
        }
        frames
    }

    /// Whether `offset` (relative to the load base) is in this module's code.
    pub(crate) fn contains(&self, offset: u64) -> bool {
        self.code
            .iter()
            .any(|&(start, end)| (start..end).contains(&offset))
    }

    fn unwind_cfi(
        &self,
        regs: &FrameRegs,
        load_base: u64,
        mem: &dyn TargetMemory,
    ) -> Option<FrameRegs> {
        let offset = regs.pc().wrapping_sub(load_base);
        if !self.contains(offset) {
            return None;
        }
        self.with_unwind_row(offset, |row| apply_row(row, regs, mem))
    }

    /// Runs `f` on the CFI row for module-relative `offset`, from
    /// `.eh_frame` first, then `.debug_frame`; the first `Some` wins.
    pub(crate) fn with_unwind_row<T>(
        &self,
        offset: u64,
        mut f: impl FnMut(&gimli::UnwindTableRow<usize>) -> Option<T>,
    ) -> Option<T> {
        let mut ctx = gimli::UnwindContext::new();
        if let Some((eh_frame, bases)) = &self.eh_frame {
            if let Ok(row) = eh_frame.unwind_info_for_address(
                bases,
                &mut ctx,
                offset,
                gimli::EhFrame::cie_from_offset,
            ) {
                if let Some(out) = f(row) {
                    return Some(out);
                }
            }
        }
        let debug_frame = self.debug_frame.as_ref()?;
        let row = debug_frame
            .unwind_info_for_address(
                &gimli::BaseAddresses::default(),
                &mut ctx,
                offset,
                gimli::DebugFrame::cie_from_offset,
            )
            .ok()?;
        f(row)
    }
}

fn apply_row(
    row: &gimli::UnwindTableRow<usize>,
    regs: &FrameRegs,
    mem: &dyn TargetMemory,
) -> Option<FrameRegs> {
    let cfa = match row.cfa() {
        gimli::CfaRule::RegisterAndOffset { register, offset } => {
            regs.get(register.0)?.wrapping_add_signed(*offset)
        }
        gimli::CfaRule::Expression(_) => return None,
    };
    let recover = |reg: u16| -> Option<u64> {
        match row.register(gimli::Register(reg)) {
            None | Some(RegisterRule::SameValue) => regs.get(reg),
            Some(RegisterRule::Offset(n)) => read_u64(mem, cfa.wrapping_add_signed(n)),
            Some(RegisterRule::ValOffset(n)) => Some(cfa.wrapping_add_signed(n)),
            Some(RegisterRule::Register(r)) => regs.get(r.0),
            Some(RegisterRule::Constant(v)) => Some(v),
            Some(_) => None,
        }
    };
    let ret = match row.register(gimli::Register(x86_64::RIP)) {
        // The return address column always has a rule in practice; without
        // one, assume the plain `call` layout.
        None => read_u64(mem, cfa.wrapping_sub(WORD_SIZE as u64))?,
        Some(RegisterRule::SameValue) => return None,
        Some(_) => recover(x86_64::RIP)?,
    };
    let mut caller = FrameRegs::default()
        .with(x86_64::RSP, cfa)
        .with(x86_64::RIP, ret.wrapping_sub(1));
    for reg in CALLEE_SAVED {
        if let Some(value) = recover(reg) {
            caller.set(reg, value);
        }
    }
    Some(caller)
}

/// `push rbp; mov rbp, rsp` frames: saved `rbp` at `[rbp]`, return address
/// right above it.
fn unwind_frame_pointer(regs: &FrameRegs, mem: &dyn TargetMemory) -> Option<FrameRegs> {
    let fp = regs.fp()?;
    if fp == 0 || fp % WORD_SIZE as u64 != 0 {
        return None;
    }
    let saved_fp = read_u64(mem, fp)?;
    let ret = read_u64(mem, fp.wrapping_add(WORD_SIZE as u64))?;
    Some(FrameRegs::new(
        ret.wrapping_sub(1),
        fp.wrapping_add(FP_TO_CFA),
        saved_fp,
    ))
}

fn read_u64(mem: &dyn TargetMemory, addr: u64) -> Option<u64> {
    let mut buf = [0u8; WORD_SIZE];
    mem.read(addr, &mut buf).then(|| u64::from_le_bytes(buf))
}
