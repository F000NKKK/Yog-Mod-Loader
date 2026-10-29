//! Local variables and parameters: scope lookup (`DW_TAG_subprogram` /
//! `lexical_block` / `inlined_subroutine` containing a pc) and evaluation of
//! their `DW_AT_location`s against a stopped frame the caller describes.
//!
//! Nothing in here touches a live process directly — registers come in as a
//! [`FrameRegs`] and memory through a caller-implemented [`TargetMemory`],
//! so `yog-debugger` (ptrace) and anything else (core dumps, tests) can drive
//! it the same way.

use std::collections::{HashMap, HashSet};
use std::sync::{Mutex, OnceLock};

use gimli::{AttributeValue, DebuggingInformationEntry, Reader as _, UnitOffset};

use crate::unwinding::arch::{Arch, SLOTS, aarch64, x86_64};
use crate::{R, SymbolTable};

mod location;

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

/// Bytes in a pointer / general-purpose register.
pub(crate) const WORD_SIZE: usize = 8;
/// Offset of the CFA from `rbp` in a `push rbp; mov rbp, rsp` frame: the
/// saved `rbp` and the return address sit between them.
pub(crate) const FP_TO_CFA: u64 = 16;
/// Largest value [`Place::read`] reads, so a corrupt length can't make us
/// allocate gigabytes.
const MAX_READ: usize = 1 << 20;
/// Largest piece of a composite location.
const MAX_PIECE: usize = 1 << 16;
/// Step limit for one DWARF expression (they can loop).
const MAX_EXPR_STEPS: u32 = 10_000;
/// `DW_AT_abstract_origin` / `DW_AT_specification` links followed.
const MAX_ORIGIN_CHAIN: usize = 4;

/// Register state of one stopped frame on machine `arch`, keyed by DWARF
/// register number (see [`crate::unwinding::arch`]). Registers the caller doesn't
/// know (e.g. caller-saved registers in an outer frame of a backtrace) stay
/// `None`, and anything located in them reads back as `<unavailable>`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FrameRegs {
    arch: Arch,
    regs: [Option<u64>; SLOTS],
}

impl Default for FrameRegs {
    /// A frame of the host's machine that knows no register yet.
    fn default() -> Self {
        FrameRegs::empty(Arch::host())
    }
}

impl FrameRegs {
    /// The minimum a frame needs: absolute pc, stack pointer and frame pointer.
    pub fn new(arch: Arch, pc: u64, sp: u64, fp: u64) -> Self {
        FrameRegs::empty(arch)
            .with(arch.pc(), pc)
            .with(arch.sp(), sp)
            .with(arch.fp(), fp)
    }

    /// A frame of `arch` that knows no register yet.
    pub fn empty(arch: Arch) -> Self {
        FrameRegs {
            arch,
            regs: [None; SLOTS],
        }
    }

    /// All 17 x86_64 registers in DWARF order (`rax, rdx, rcx, rbx, rsi,
    /// rdi, rbp, rsp, r8..r15, rip`).
    pub fn x86_64(values: [u64; x86_64::COUNT]) -> Self {
        let mut frame = FrameRegs::empty(Arch::X86_64);
        for (slot, v) in frame.regs.iter_mut().zip(values) {
            *slot = Some(v);
        }
        frame
    }

    /// AArch64's `x0`..`x30`, stack pointer and pc.
    pub fn aarch64(x: [u64; aarch64::GENERAL], sp: u64, pc: u64) -> Self {
        let mut frame = FrameRegs::empty(Arch::Aarch64);
        for (slot, v) in frame.regs.iter_mut().zip(x) {
            *slot = Some(v);
        }
        frame.with(aarch64::SP, sp).with(aarch64::PC, pc)
    }

    /// The machine its registers are of.
    pub fn arch(&self) -> Arch {
        self.arch
    }

    /// Sets DWARF register `dwarf_reg`; numbers outside the supported set
    /// are ignored.
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

    /// Absolute pc, or 0 if unset.
    pub fn pc(&self) -> u64 {
        self.get(self.arch.pc()).unwrap_or(0)
    }

    pub fn sp(&self) -> Option<u64> {
        self.get(self.arch.sp())
    }

    pub fn fp(&self) -> Option<u64> {
        self.get(self.arch.fp())
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
    /// Whether the pc lies inside the variable's lexical scope. `false`
    /// for a variable declared in the frame's innermost block that isn't
    /// live yet — e.g. stopped on `let x = ...;` itself, since rustc's
    /// scope for `x` only starts once the initializer has run. Such
    /// variables read back as `<not in scope>` rather than garbage.
    pub in_scope: bool,
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
pub(crate) const NOT_IN_SCOPE: &str = "<not in scope>";

/// One compilation unit, parsed on first use.
pub(crate) struct UnitInfo {
    pub(crate) unit: gimli::Unit<R>,
    ranges: Vec<gimli::Range>,
    /// child offset -> parent offset, built lazily for qualified type names.
    parents: OnceLock<HashMap<usize, usize>>,
    /// Out-of-line functions with their code ranges, in DIE order, built on
    /// the first locals lookup in this unit.
    functions: OnceLock<Vec<(gimli::Range, UnitOffset<usize>)>>,
    /// DIE offset -> qualified name, filled as names are asked for (every
    /// value of a struct type asks again).
    pub(crate) qualified_names: Mutex<HashMap<usize, String>>,
}

impl UnitInfo {
    pub(crate) fn parents(&self) -> &HashMap<usize, usize> {
        self.parents.get_or_init(|| {
            let mut map = HashMap::new();
            let mut stack: Vec<usize> = Vec::new();
            let mut cursor = self.unit.entries();
            while let Ok(Some(entry)) = cursor.next_dfs() {
                let depth = entry.depth().max(0) as usize;
                stack.truncate(depth);
                if let Some(&parent) = stack.last() {
                    map.insert(entry.offset().0, parent);
                }
                stack.push(entry.offset().0);
            }
            map
        })
    }
}

/// Where a variable's bytes live once its location has been evaluated.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Place {
    /// In target memory at this absolute address.
    Mem(u64),
    /// Materialized (registers, implicit values, composed pieces).
    Bytes(std::sync::Arc<[u8]>, usize),
}

impl Place {
    pub(crate) fn offset(&self, by: u64) -> Place {
        match self {
            Place::Mem(addr) => Place::Mem(addr.wrapping_add(by)),
            Place::Bytes(bytes, off) => {
                Place::Bytes(bytes.clone(), off.saturating_add(by as usize))
            }
        }
    }

    /// Reads `len` bytes at this place; `None` if any of it is unavailable.
    pub(crate) fn read(&self, mem: &dyn TargetMemory, len: usize) -> Option<Vec<u8>> {
        if len > MAX_READ {
            return None;
        }
        match self {
            Place::Mem(addr) => {
                let mut buf = vec![0u8; len];
                if len == 0 || mem.read(*addr, &mut buf) {
                    Some(buf)
                } else {
                    None
                }
            }
            Place::Bytes(bytes, off) => bytes.get(*off..off.checked_add(len)?).map(<[u8]>::to_vec),
        }
    }
}

pub(crate) enum Located {
    Place(Place),
    OptimizedOut,
    Unavailable,
}

/// Everything location evaluation needs about the stopped frame.
pub(crate) struct Frame<'a> {
    pub(crate) regs: &'a FrameRegs,
    pub(crate) mem: &'a dyn TargetMemory,
    pub(crate) load_base: u64,
    /// Module-relative pc.
    pub(crate) pc: u64,
}

impl<'a> Frame<'a> {
    pub(crate) fn new(regs: &'a FrameRegs, mem: &'a dyn TargetMemory, load_base: u64) -> Self {
        Frame {
            regs,
            mem,
            load_base,
            pc: regs.pc().wrapping_sub(load_base),
        }
    }
}

impl SymbolTable {
    pub(crate) fn units(&self) -> &[UnitInfo] {
        self.units.get_or_init(|| {
            let mut out = Vec::new();
            let mut headers = self.dwarf.units();
            while let Ok(Some(header)) = headers.next() {
                let Ok(unit) = self.dwarf.unit(header) else {
                    continue;
                };
                let mut ranges = Vec::new();
                if let Ok(mut iter) = self.dwarf.unit_ranges(&unit) {
                    while let Ok(Some(range)) = iter.next() {
                        if range.begin < range.end {
                            ranges.push(range);
                        }
                    }
                }
                out.push(UnitInfo {
                    unit,
                    ranges,
                    parents: OnceLock::new(),
                    qualified_names: Mutex::default(),
                    functions: OnceLock::new(),
                });
            }
            out
        })
    }

    /// The outermost `DW_TAG_subprogram` containing `pc` in `info`'s unit —
    /// where [`SymbolTable::walk_scope`] starts — from an index of every
    /// function's ranges, so a lookup doesn't rescan the whole unit.
    fn function_for_pc(&self, info: &UnitInfo, pc: u64) -> Option<UnitOffset<usize>> {
        let functions = info.functions.get_or_init(|| {
            let mut out = Vec::new();
            let mut cursor = info.unit.entries();
            // Depth of the function being skipped over, if inside one:
            // nested functions are found by the scope walk itself.
            let mut inside: Option<isize> = None;
            while let Ok(Some(entry)) = cursor.next_dfs() {
                let depth = entry.depth();
                if inside.is_some_and(|d| depth > d) {
                    continue;
                }
                inside = None;
                if entry.tag() != gimli::DW_TAG_subprogram {
                    continue;
                }
                inside = Some(depth);
                if let Ok(mut ranges) = self.dwarf.die_ranges(&info.unit, entry) {
                    while let Ok(Some(range)) = ranges.next() {
                        out.push((range, entry.offset()));
                    }
                }
            }
            out
        });
        functions
            .iter()
            .find(|(r, _)| r.begin <= pc && pc < r.end)
            .map(|&(_, offset)| offset)
    }

    pub(crate) fn unit_for_pc(&self, pc: u64) -> Option<usize> {
        self.units()
            .iter()
            .position(|u| u.ranges.iter().any(|r| r.begin <= pc && pc < r.end))
    }

    /// Parameters and local variables of the innermost function instance
    /// containing `pc_offset` (module relative, like
    /// [`SymbolTable::resolve_addr`]); an inlined call counts as its own
    /// function, as in gdb. Same as [`SymbolTable::locals_in_frame`] with
    /// `inline_depth` 0.
    pub fn locals(&self, pc_offset: u64) -> Vec<LocalVar> {
        self.locals_in_frame(pc_offset, 0)
    }

    /// Parameters and local variables of one function instance containing
    /// `pc_offset`: `inline_depth` 0 is the innermost (an inlined call, if
    /// the pc is in one), 1 its caller, and so on out to the out-of-line
    /// function — the same order as [`SymbolTable::resolve_inlined`].
    ///
    /// Returns the parameters, then the variables in scope from the
    /// outermost to the innermost enclosing lexical block (a variable
    /// shadowed by a same-named one in an inner block is omitted), then
    /// the variables declared further down the innermost block that aren't
    /// live yet ([`LocalVar::in_scope`] `false`; names already listed are
    /// skipped). Empty if the pc has no such frame.
    ///
    /// For outer frames of a backtrace, pass `return_address - 1` so the
    /// lookup lands inside the call instruction's scope.
    pub fn locals_in_frame(&self, pc_offset: u64, inline_depth: usize) -> Vec<LocalVar> {
        let Some(unit_idx) = self.unit_for_pc(pc_offset) else {
            return Vec::new();
        };
        let info = &self.units()[unit_idx];
        let Some(function) = self.function_for_pc(info, pc_offset) else {
            return Vec::new();
        };
        let Ok(mut tree) = info.unit.entries_tree(Some(function)) else {
            return Vec::new();
        };
        let Ok(root) = tree.root() else {
            return Vec::new();
        };
        let mut walk = Walk {
            subprogram: function,
            path: vec![Scope {
                offset: function,
                is_function: true,
                vars: Vec::new(),
            }],
        };
        let _ = self.walk_scope(info, root, pc_offset, &mut walk);
        let subprogram = walk.subprogram;

        // The scopes of the requested function instance: from its own
        // subprogram/inlined_subroutine to just before the next one in.
        let starts: Vec<usize> = (0..walk.path.len())
            .filter(|&i| walk.path[i].is_function)
            .collect();
        let Some(pos) = starts.len().checked_sub(inline_depth + 1) else {
            return Vec::new();
        };
        let begin = starts[pos];
        let end = starts.get(pos + 1).copied().unwrap_or(walk.path.len());
        let scopes = &walk.path[begin..end];

        let at = VarsIn {
            unit: unit_idx,
            info,
            subprogram,
        };
        let mut seen = HashSet::new();
        let mut vars = self.visible_vars(&at, scopes, &mut seen);
        // Declared but not live yet: rustc opens a nested lexical block per
        // `let`, starting after its initializer, so these hang below the
        // innermost scope containing the pc.
        let mut pending = Vec::new();
        if let Some(innermost) = scopes.last()
            && let Ok(mut tree) = info.unit.entries_tree(Some(innermost.offset))
            && let Ok(node) = tree.root()
        {
            let _ = collect_block_vars(node, false, &mut pending);
        }
        for die in pending {
            if let Some(var) = self.describe_var(&at, die, VarKind::Local, false)
                && seen.insert(var.name.clone())
            {
                vars.push(var);
            }
        }
        vars
    }

    /// The variables of `scopes` (outermost first), parameters first; an
    /// inner scope's shadows an outer one's of the same name. Their names go to `seen`.
    fn visible_vars(
        &self,
        at: &VarsIn<'_>,
        scopes: &[Scope],
        seen: &mut HashSet<String>,
    ) -> Vec<LocalVar> {
        // Inner scopes shadow outer ones: walk inside-out, keep first seen.
        let mut keep: Vec<Vec<LocalVar>> = Vec::with_capacity(scopes.len());
        for scope in scopes.iter().rev() {
            let mut kept = Vec::new();
            for &(kind, die) in scope.vars.iter().rev() {
                if let Some(var) = self.describe_var(at, die, kind, true)
                    && seen.insert(var.name.clone())
                {
                    kept.push(var);
                }
            }
            kept.reverse();
            keep.push(kept);
        }
        keep.reverse();
        let (mut params, locals): (Vec<_>, Vec<_>) = keep
            .into_iter()
            .flatten()
            .partition(|v| v.kind == VarKind::Parameter);
        params.extend(locals);
        params
    }

    /// [`SymbolTable::locals`] at the frame's pc, each read with
    /// [`SymbolTable::read_local`].
    pub fn read_locals(
        &self,
        regs: &FrameRegs,
        load_base: u64,
        mem: &dyn TargetMemory,
    ) -> Vec<Value> {
        self.read_locals_in_frame(regs, load_base, mem, 0)
    }

    /// [`SymbolTable::locals_in_frame`] at the frame's pc, each read with
    /// [`SymbolTable::read_local`]. For the callers of an inlined call:
    /// they share the physical frame's registers.
    pub fn read_locals_in_frame(
        &self,
        regs: &FrameRegs,
        load_base: u64,
        mem: &dyn TargetMemory,
        inline_depth: usize,
    ) -> Vec<Value> {
        let pc_offset = regs.pc().wrapping_sub(load_base);
        self.locals_in_frame(pc_offset, inline_depth)
            .iter()
            .map(|var| self.read_local(var, regs, load_base, mem))
            .collect()
    }

    /// Evaluates `var`'s location in the frame described by `regs` (whose
    /// pc is absolute; `load_base` is the module's runtime load bias, so
    /// `regs.pc() - load_base` is the module-relative pc) and formats its
    /// value. Never panics on bad memory or DWARF: unreadable parts come
    /// back as `<unavailable>`, absent ones as `<optimized out>`, and a
    /// variable that isn't live yet ([`LocalVar::in_scope`] `false`) as
    /// `<not in scope>` without being read.
    pub fn read_local(
        &self,
        var: &LocalVar,
        regs: &FrameRegs,
        load_base: u64,
        mem: &dyn TargetMemory,
    ) -> Value {
        if !var.in_scope {
            return leaf(var, NOT_IN_SCOPE);
        }
        let frame = Frame::new(regs, mem, load_base);
        let Some(info) = self.units().get(var.unit) else {
            return leaf(var, UNAVAILABLE);
        };
        let located = match info.unit.entry(var.die) {
            Ok(entry) => self.locate(info, &entry, var.subprogram, &frame),
            Err(_) => Located::Unavailable,
        };
        match located {
            Located::Place(place) => {
                let fmt = crate::values::format::Formatter::new(self, info, mem);
                let mut value = fmt.format(var.name.clone(), var.ty, &place, 0);
                if value.type_name.is_empty() {
                    value.type_name = crate::values::format::short_type_name(&var.type_name);
                }
                value
            }
            Located::OptimizedOut => leaf(var, OPTIMIZED_OUT),
            Located::Unavailable => leaf(var, UNAVAILABLE),
        }
    }

    /// Collects the scopes below `node` (a function scope) that contain
    /// `pc`, with the variables declared directly in each, into `walk`.
    fn walk_scope(
        &self,
        info: &UnitInfo,
        node: gimli::EntriesTreeNode<'_, '_, R>,
        pc: u64,
        walk: &mut Walk,
    ) -> gimli::Result<()> {
        let my_scope = walk.path.len().checked_sub(1);
        let mut children = node.children();
        while let Some(child) = children.next()? {
            let entry = child.entry();
            match entry.tag() {
                gimli::DW_TAG_formal_parameter | gimli::DW_TAG_variable => {
                    let kind = if entry.tag() == gimli::DW_TAG_formal_parameter {
                        VarKind::Parameter
                    } else {
                        VarKind::Local
                    };
                    if let Some(scope) = my_scope.and_then(|i| walk.path.get_mut(i)) {
                        scope.vars.push((kind, entry.offset()));
                    }
                }
                gimli::DW_TAG_subprogram
                | gimli::DW_TAG_lexical_block
                | gimli::DW_TAG_inlined_subroutine => {
                    if !self.die_contains(info, entry, pc) {
                        continue;
                    }
                    if entry.tag() == gimli::DW_TAG_subprogram {
                        walk.subprogram = entry.offset();
                    }
                    walk.path.push(Scope {
                        offset: entry.offset(),
                        is_function: entry.tag() != gimli::DW_TAG_lexical_block,
                        vars: Vec::new(),
                    });
                    self.walk_scope(info, child, pc, walk)?;
                }
                _ => {}
            }
        }
        Ok(())
    }

    fn die_contains(&self, info: &UnitInfo, entry: &DebuggingInformationEntry<R>, pc: u64) -> bool {
        let Ok(mut ranges) = self.dwarf.die_ranges(&info.unit, entry) else {
            return false;
        };
        while let Ok(Some(r)) = ranges.next() {
            if r.begin <= pc && pc < r.end {
                return true;
            }
        }
        false
    }

    fn describe_var(
        &self,
        at: &VarsIn<'_>,
        die: UnitOffset<usize>,
        kind: VarKind,
        in_scope: bool,
    ) -> Option<LocalVar> {
        let info = at.info;
        let entry = info.unit.entry(die).ok()?;
        let name_attr = self.attr_with_origin(info, &entry, gimli::DW_AT_name)?;
        let name = self
            .dwarf
            .attr_string(&info.unit, name_attr)
            .ok()?
            .to_string_lossy()
            .ok()?
            .into_owned();
        let ty = self
            .attr_with_origin(info, &entry, gimli::DW_AT_type)
            .and_then(|v| unit_ref(info, v));
        let decl_line = self
            .attr_with_origin(info, &entry, gimli::DW_AT_decl_line)
            .and_then(|v| v.udata_value())
            .map(|l| l as u32);
        let fmt = crate::values::format::Formatter::new(self, info, &NoMemory);
        Some(LocalVar {
            name,
            type_name: fmt.type_name(ty),
            kind,
            decl_line,
            in_scope,
            unit: at.unit,
            die,
            ty,
            subprogram: at.subprogram,
        })
    }
}

/// Where variables are described: their unit (its index and info) and the function they're in.
struct VarsIn<'a> {
    unit: usize,
    info: &'a UnitInfo,
    subprogram: UnitOffset<usize>,
}

struct Walk {
    /// The concrete (out-of-line) subprogram containing the pc.
    subprogram: UnitOffset<usize>,
    /// Every scope containing the pc, outermost first.
    path: Vec<Scope>,
}

/// A `DW_TAG_subprogram` / `inlined_subroutine` / `lexical_block`
/// containing the pc, with the parameters and variables declared directly
/// in it.
struct Scope {
    offset: UnitOffset<usize>,
    /// Subprogram or inlined call: starts a new function instance.
    is_function: bool,
    vars: Vec<(VarKind, UnitOffset<usize>)>,
}

/// Variables in the lexical blocks below `node` (pre-order, i.e.
/// declaration order), not counting `node`'s own and not descending into
/// nested function instances (inlined calls).
fn collect_block_vars(
    node: gimli::EntriesTreeNode<'_, '_, R>,
    nested: bool,
    out: &mut Vec<UnitOffset<usize>>,
) -> gimli::Result<()> {
    let mut children = node.children();
    while let Some(child) = children.next()? {
        match child.entry().tag() {
            gimli::DW_TAG_variable if nested => out.push(child.entry().offset()),
            gimli::DW_TAG_lexical_block => collect_block_vars(child, true, out)?,
            _ => {}
        }
    }
    Ok(())
}

/// Resolves a type/origin reference to an offset within `info`'s unit
/// (cross-unit references are not followed).
pub(crate) fn unit_ref(info: &UnitInfo, v: AttributeValue<R>) -> Option<UnitOffset<usize>> {
    match v {
        AttributeValue::UnitRef(off) => Some(off),
        AttributeValue::DebugInfoRef(off) => off.to_unit_offset(&info.unit.header),
        _ => None,
    }
}

fn leaf(var: &LocalVar, value: &str) -> Value {
    Value {
        name: var.name.clone(),
        type_name: crate::values::format::short_type_name(&var.type_name),
        value: value.to_string(),
        children: Vec::new(),
    }
}

/// For computing type names without a target.
pub(crate) struct NoMemory;

impl TargetMemory for NoMemory {
    fn read(&self, _addr: u64, _buf: &mut [u8]) -> bool {
        false
    }
}
