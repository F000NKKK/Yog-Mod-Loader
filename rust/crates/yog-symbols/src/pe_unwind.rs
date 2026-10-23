//! Unwinding Windows x64 code by its exception tables: `.pdata`'s function
//! entries and the `UNWIND_INFO` each points to — what Windows itself
//! unwinds by (`RtlVirtualUnwind`), and the only frame description a
//! `windows-gnu` native has (no `.eh_frame`).

use object::{Object as _, ObjectSection as _};

use crate::arch::{Arch, x86_64};
use crate::locals::{FrameRegs, TargetMemory, WORD_SIZE};

/// A `.pdata` entry: the function's RVAs and its unwind info's.
const ENTRY: usize = 12;
/// An `UNWIND_INFO`'s header, then its codes (two bytes each).
const HEADER: usize = 4;
const CODE: usize = 2;
/// Its flags (above the version's three bits): its codes go on in another
/// function's (`UNW_FLAG_CHAININFO`).
const FLAGS_SHIFT: u8 = 3;
const CHAINED: u8 = 4;
/// Its frame register (low four bits) and that register's offset, in 16s.
const NIBBLE: u8 = 0xF;
const FRAME_SCALE: u64 = 16;
/// A machine frame's stack pointer, in words above its pc.
const MACHFRAME_SP: u64 = 3;
/// Chains followed at most (a corrupt table can't loop).
const MAX_CHAIN: usize = 32;
/// The sections the tables live in.
const TABLES: [&str; 3] = [".pdata", ".xdata", ".rdata"];

/// The unwind operations.
mod op {
    pub const PUSH_NONVOL: u8 = 0;
    pub const ALLOC_LARGE: u8 = 1;
    pub const ALLOC_SMALL: u8 = 2;
    pub const SET_FPREG: u8 = 3;
    pub const SAVE_NONVOL: u8 = 4;
    pub const SAVE_NONVOL_FAR: u8 = 5;
    pub const SAVE_XMM128: u8 = 8;
    pub const SAVE_XMM128_FAR: u8 = 9;
    pub const PUSH_MACHFRAME: u8 = 10;
}

/// Windows' register numbers (`rax, rcx, rdx, rbx, rsp, rbp, rsi, rdi,
/// r8..r15`) as DWARF's.
const DWARF_OF: [u16; 16] = [
    x86_64::RAX,
    x86_64::RCX,
    x86_64::RDX,
    x86_64::RBX,
    x86_64::RSP,
    x86_64::RBP,
    x86_64::RSI,
    x86_64::RDI,
    x86_64::R8,
    x86_64::R9,
    x86_64::R10,
    x86_64::R11,
    x86_64::R12,
    x86_64::R13,
    x86_64::R14,
    x86_64::R15,
];

/// Registers Windows' x64 ABI keeps for a caller.
const NONVOLATILE: [u16; 8] = [
    x86_64::RBX,
    x86_64::RBP,
    x86_64::RDI,
    x86_64::RSI,
    x86_64::R12,
    x86_64::R13,
    x86_64::R14,
    x86_64::R15,
];

/// A PE native's exception tables.
pub(crate) struct PeUnwind {
    /// `.pdata`: (begin, end, unwind info) RVAs, sorted by begin.
    functions: Vec<(u32, u32, u32)>,
    /// The sections unwind info may be in: their RVA and bytes.
    sections: Vec<(u32, Vec<u8>)>,
}

fn u16_at(b: &[u8], at: usize) -> Option<u16> {
    Some(u16::from_le_bytes(b.get(at..at + 2)?.try_into().ok()?))
}

fn u32_at(b: &[u8], at: usize) -> Option<u32> {
    Some(u32::from_le_bytes(b.get(at..at + 4)?.try_into().ok()?))
}

impl PeUnwind {
    /// `object`'s tables, when it's a PE that has them.
    pub(crate) fn of(object: &object::File<'_>) -> Option<PeUnwind> {
        if object.format() != object::BinaryFormat::Pe {
            return None;
        }
        let base = object.relative_address_base();
        let mut sections = Vec::new();
        for name in TABLES {
            if let Some(s) = object.section_by_name(name)
                && let Ok(data) = s.data()
            {
                sections.push(((s.address() - base) as u32, data.to_vec()));
            }
        }
        let pdata = &sections.first()?.1;
        let mut functions: Vec<(u32, u32, u32)> = pdata
            .as_chunks::<ENTRY>()
            .0
            .iter()
            .filter_map(|e| Some((u32_at(e, 0)?, u32_at(e, 4)?, u32_at(e, 8)?)))
            .collect();
        functions.sort_unstable();
        Some(PeUnwind {
            functions,
            sections,
        })
    }

    /// The bytes at `rva`, as far as their section goes.
    fn at(&self, rva: u32) -> Option<&[u8]> {
        self.sections.iter().find_map(|(start, data)| {
            let off = rva.checked_sub(*start)? as usize;
            data.get(off..)
        })
    }

    fn function(&self, rva: u32) -> Option<(u32, u32, u32)> {
        let i = self
            .functions
            .partition_point(|f| f.0 <= rva)
            .checked_sub(1)?;
        let f = self.functions[i];
        (rva < f.1).then_some(f)
    }

    /// The caller of the frame `regs` describes, its pc at `rva`; `None`
    /// when the tables don't say (or memory doesn't read).
    pub(crate) fn unwind(
        &self,
        regs: &FrameRegs,
        rva: u32,
        mem: &dyn TargetMemory,
    ) -> Option<FrameRegs> {
        let mut caller = FrameRegs::empty(Arch::X86_64);
        for reg in NONVOLATILE {
            if let Some(v) = regs.get(reg) {
                caller.set(reg, v);
            }
        }
        let mut sp = regs.sp()?;
        let Some((begin, _, info)) = self.function(rva) else {
            // A leaf: nothing pushed, the return address on top.
            return finish(caller, sp, mem);
        };
        let mut unwound = Unwound {
            regs: &mut caller,
            sp: &mut sp,
            mem,
            from: regs,
        };
        let mut at = Some((info, rva - begin));
        for _ in 0..MAX_CHAIN {
            let Some((info, in_prolog)) = at else { break };
            match unwound.apply(self.at(info)?, in_prolog)? {
                Applied::Done => return Some(caller),
                Applied::Chained(next) => at = Some((next, u32::MAX)),
                Applied::Return => break,
            }
        }
        finish(caller, sp, mem)
    }
}

/// The return address on top of the stack at `sp`: the caller's pc and stack.
fn finish(caller: FrameRegs, sp: u64, mem: &dyn TargetMemory) -> Option<FrameRegs> {
    let ret = read(mem, sp)?;
    Some(
        caller
            .with(x86_64::RIP, ret.wrapping_sub(1))
            .with(x86_64::RSP, sp + WORD_SIZE as u64),
    )
}

fn read(mem: &dyn TargetMemory, addr: u64) -> Option<u64> {
    let mut buf = [0u8; WORD_SIZE];
    mem.read(addr, &mut buf).then(|| u64::from_le_bytes(buf))
}

/// What an `UNWIND_INFO` came to.
enum Applied {
    /// A machine frame: the caller is wholly known.
    Done,
    /// Its codes go on in this unwind info.
    Chained(u32),
    /// Its codes are done: the return address is on top.
    Return,
}

/// An unwind under way: the caller's registers and stack pointer so far.
struct Unwound<'a> {
    regs: &'a mut FrameRegs,
    sp: &'a mut u64,
    mem: &'a dyn TargetMemory,
    /// The frame being unwound (its frame register's value).
    from: &'a FrameRegs,
}

/// A function's frame register and where its frame is.
struct Frame {
    /// Where `SAVE_NONVOL`s count from.
    base: u64,
    /// The register `SET_FPREG` set, and how far below it the stack was.
    reg: u16,
    offset: u64,
}

/// One unwind code: its operation and argument, where it is in its info,
/// and whether the pc is past it (it has happened).
struct Code {
    op: u8,
    arg: u8,
    at: usize,
    applies: bool,
}

impl Unwound<'_> {
    /// Runs `info`'s codes — those the pc, `in_prolog` bytes into the
    /// function, is past.
    fn apply(&mut self, info: &[u8], in_prolog: u32) -> Option<Applied> {
        let (flags, count, frame_byte) = (
            *info.first()? >> FLAGS_SHIFT,
            usize::from(*info.get(2)?),
            *info.get(3)?,
        );
        let reg = DWARF_OF[usize::from(frame_byte & NIBBLE)];
        let offset = u64::from(frame_byte >> 4) * FRAME_SCALE;
        let base = match frame_byte & NIBBLE {
            0 => *self.sp,
            _ => self.from.get(reg)?.wrapping_sub(offset),
        };
        let frame = Frame { base, reg, offset };
        let mut i = 0;
        while i < count {
            let at = HEADER + i * CODE;
            let (offset, byte) = (*info.get(at)?, *info.get(at + 1)?);
            let code = Code {
                op: byte & NIBBLE,
                arg: byte >> 4,
                at,
                applies: u32::from(offset) <= in_prolog,
            };
            let slots = self.op(&code, info, &frame)?;
            if slots == 0 {
                return Some(Applied::Done);
            }
            i += slots;
        }
        if flags & CHAINED != 0 {
            let chain = HEADER + (count + count % 2) * CODE;
            return Some(Applied::Chained(u32_at(info, chain + 8)?));
        }
        Some(Applied::Return)
    }

    /// Code `c` of `info`; the slots it takes, 0 for a machine frame (the
    /// caller then known).
    fn op(&mut self, c: &Code, info: &[u8], frame: &Frame) -> Option<usize> {
        let (word, applies) = (WORD_SIZE as u64, c.applies);
        let reg = DWARF_OF[usize::from(c.arg)];
        let next16 = || u16_at(info, c.at + CODE).map(u64::from);
        let next32 = || u32_at(info, c.at + CODE).map(u64::from);
        Some(match c.op {
            op::PUSH_NONVOL => {
                if applies {
                    self.regs.set(reg, read(self.mem, *self.sp)?);
                    *self.sp += word;
                }
                1
            }
            op::ALLOC_LARGE if c.arg == 0 => {
                if applies {
                    *self.sp += next16()? * word;
                }
                2
            }
            op::ALLOC_LARGE => {
                if applies {
                    *self.sp += next32()?;
                }
                3
            }
            op::ALLOC_SMALL => {
                if applies {
                    *self.sp += u64::from(c.arg) * word + word;
                }
                1
            }
            op::SET_FPREG => {
                if applies {
                    *self.sp = self.from.get(frame.reg)?.wrapping_sub(frame.offset);
                }
                1
            }
            op::SAVE_NONVOL => {
                if applies {
                    self.regs
                        .set(reg, read(self.mem, frame.base + next16()? * word)?);
                }
                2
            }
            op::SAVE_NONVOL_FAR => {
                if applies {
                    self.regs.set(reg, read(self.mem, frame.base + next32()?)?);
                }
                3
            }
            op::SAVE_XMM128 => 2,
            op::SAVE_XMM128_FAR => 3,
            op::PUSH_MACHFRAME => {
                // Its error code first, when the argument says there is one.
                let top = *self.sp + u64::from(c.arg) * word;
                let rip = read(self.mem, top)?;
                let rsp = read(self.mem, top + MACHFRAME_SP * word)?;
                self.regs.set(x86_64::RIP, rip.wrapping_sub(1));
                self.regs.set(x86_64::RSP, rsp);
                0
            }
            // Epilog descriptions (version 2) and the like: one slot each.
            _ => 1,
        })
    }
}
