//! Local variables and parameters: scope lookup (`DW_TAG_subprogram` /
//! `lexical_block` / `inlined_subroutine` containing a pc) and evaluation of
//! their `DW_AT_location`s against a stopped frame the caller describes.
//!
//! Nothing in here touches a live process directly — registers come in as a
//! [`FrameRegs`] and memory through a caller-implemented [`TargetMemory`],
//! so `yog-debugger` (ptrace) and anything else (core dumps, tests) can drive
//! it the same way.

use std::collections::HashMap;
use std::sync::OnceLock;

use gimli::{AttributeValue, EvaluationResult, Location, Piece, UnitOffset, UnwindSection as _};

use crate::{R, SymbolTable};

/// Read access to the stopped target's memory, implemented by the caller
/// (e.g. via `process_vm_readv` or `/proc/<pid>/mem`). Addresses are
/// absolute runtime addresses.
pub trait TargetMemory {
    /// Fills `buf` from `addr`. Returns `false` if any of it is unreadable.
    fn read(&self, addr: u64, buf: &mut [u8]) -> bool;
}

impl<T: TargetMemory + ?Sized> TargetMemory for &T {
    fn read(&self, addr: u64, buf: &mut [u8]) -> bool {
        (**self).read(addr, buf)
    }
}

/// DWARF register numbers for x86_64 (System V psABI), as used by
/// [`FrameRegs::set`] / [`FrameRegs::get`].
pub mod x86_64 {
    pub const RAX: u16 = 0;
    pub const RDX: u16 = 1;
    pub const RCX: u16 = 2;
    pub const RBX: u16 = 3;
    pub const RSI: u16 = 4;
    pub const RDI: u16 = 5;
    pub const RBP: u16 = 6;
    pub const RSP: u16 = 7;
    pub const R8: u16 = 8;
    pub const R9: u16 = 9;
    pub const R10: u16 = 10;
    pub const R11: u16 = 11;
    pub const R12: u16 = 12;
    pub const R13: u16 = 13;
    pub const R14: u16 = 14;
    pub const R15: u16 = 15;
    /// Return-address column; holds the frame's pc.
    pub const RIP: u16 = 16;
}

const NUM_REGS: usize = 17;

/// Register state of one stopped frame, keyed by DWARF register number
/// (see [`x86_64`]). Registers the caller doesn't know (e.g. caller-saved
/// registers in an outer frame of a backtrace) stay `None`, and anything
/// located in them reads back as `<unavailable>`.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct FrameRegs {
    regs: [Option<u64>; NUM_REGS],
}

impl FrameRegs {
    /// The minimum a frame needs: absolute pc, stack pointer (`rsp`) and
    /// frame pointer (`rbp`).
    pub fn new(pc: u64, sp: u64, fp: u64) -> Self {
        let mut regs = FrameRegs::default();
        regs.set(x86_64::RIP, pc);
        regs.set(x86_64::RSP, sp);
        regs.set(x86_64::RBP, fp);
        regs
    }

    /// All 17 x86_64 registers in DWARF order (`rax, rdx, rcx, rbx, rsi,
    /// rdi, rbp, rsp, r8..r15, rip`).
    pub fn from_dwarf_order(values: [u64; NUM_REGS]) -> Self {
        FrameRegs {
            regs: values.map(Some),
        }
    }

    /// Sets DWARF register `dwarf_reg`; numbers outside the supported set
    /// (0..=16) are ignored.
    pub fn set(&mut self, dwarf_reg: u16, value: u64) {
        if let Some(slot) = self.regs.get_mut(dwarf_reg as usize) {
            *slot = Some(value);
        }
    }

    /// Builder form of [`FrameRegs::set`].
    pub fn with(mut self, dwarf_reg: u16, value: u64) -> Self {
        self.set(dwarf_reg, value);
        self
    }

    pub fn get(&self, dwarf_reg: u16) -> Option<u64> {
        self.regs.get(dwarf_reg as usize).copied().flatten()
    }

    /// Absolute pc (`rip`), or 0 if unset.
    pub fn pc(&self) -> u64 {
        self.get(x86_64::RIP).unwrap_or(0)
    }

    pub fn sp(&self) -> Option<u64> {
        self.get(x86_64::RSP)
    }

    pub fn fp(&self) -> Option<u64> {
        self.get(x86_64::RBP)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum VarKind {
    Parameter,
    Local,
}

/// A parameter or local variable in scope at some pc, as found by
/// [`SymbolTable::locals`]. Pass it back to [`SymbolTable::read_local`] to
/// evaluate its value in a stopped frame.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LocalVar {
    pub name: String,
    pub type_name: String,
    pub kind: VarKind,
    pub decl_line: Option<u32>,
    pub(crate) unit: usize,
    pub(crate) die: UnitOffset<usize>,
    pub(crate) ty: Option<UnitOffset<usize>>,
    /// The concrete (out-of-line) `DW_TAG_subprogram` whose
    /// `DW_AT_frame_base` the variable's `DW_OP_fbreg` is relative to.
    pub(crate) subprogram: UnitOffset<usize>,
}

/// A formatted value: one row of a debugger's Variables view, with nested
/// rows for struct members, array/`Vec`/slice elements, enum variant
/// fields and pointer targets (bounded in depth and width).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Value {
    pub name: String,
    pub type_name: String,
    pub value: String,
    pub children: Vec<Value>,
}

pub(crate) const OPTIMIZED_OUT: &str = "<optimized out>";
pub(crate) const UNAVAILABLE: &str = "<unavailable>";

/// One compilation unit, parsed on first use.
pub(crate) struct UnitInfo {
    pub(crate) unit: gimli::Unit<R>,
    ranges: Vec<gimli::Range>,
    /// child offset -> parent offset, built lazily for qualified type names.
    parents: OnceLock<HashMap<usize, usize>>,
}

impl UnitInfo {
    pub(crate) fn parents(&self) -> &HashMap<usize, usize> {
        self.parents.get_or_init(|| {
            let mut map = HashMap::new();
            let mut stack: Vec<usize> = Vec::new();
            let mut cursor = self.unit.entries_raw(None);
            let Ok(mut cursor) = cursor.as_mut().map(std::mem::take_mut_placeholder) else {
                return map;
            };
            let _ = &mut cursor;
            let _ = &mut stack;
            map
        })
    }
}
