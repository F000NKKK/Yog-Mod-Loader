//! Source-level (file:line) breakpoints that survive a `yog-hot-reload`
//! reload: each one remembers which mod and [`ModuleGeneration`] it was
//! last resolved against, so [`SourceBreakpoints::rebind_after_reload`] can
//! recompute its address against the new generation's symbol table and
//! re-arm it — a function's address (or existence) can change across a
//! reload, so the old raw address is not reusable.

use yog_hot_reload::ModuleGeneration;
use yog_symbols::SymbolTable;

use crate::ptrace_debugger::{DebugError, Debugger};

pub struct SourceBreakpoint {
    pub mod_id: String,
    pub file: String,
    pub line: u32,
    pub generation: ModuleGeneration,
    /// The real runtime address currently armed in the tracee, if
    /// resolution + arming succeeded (a reload can leave this `None`
    /// briefly if the new generation no longer has a matching line).
    pub addr: Option<u64>,
}

/// Tracks every source breakpoint the IDE has asked for, independent of
/// which generation of which mod each currently resolves to.
#[derive(Default)]
pub struct SourceBreakpoints {
    breakpoints: Vec<SourceBreakpoint>,
}

impl SourceBreakpoints {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn all(&self) -> &[SourceBreakpoint] {
        &self.breakpoints
    }

    /// Resolves `file:line` against `symbols` and arms it in `debugger` at
    /// `module_base + offset`, recording it under `mod_id`/`generation` so
    /// a later reload can find and rebind it.
    pub fn set(
        &mut self,
        debugger: &mut Debugger,
        mod_id: &str,
        generation: ModuleGeneration,
        module_base: u64,
        symbols: &SymbolTable,
        file: &str,
        line: u32,
    ) -> Result<(), DebugError> {
        let addr = symbols.resolve_breakpoint(file, line).into_iter().next().map(|offset| module_base + offset);
        if let Some(addr) = addr {
            debugger.set_breakpoint(addr)?;
        }
        self.breakpoints.push(SourceBreakpoint { mod_id: mod_id.to_string(), file: file.to_string(), line, generation, addr });
        Ok(())
    }

    /// Removes every breakpoint tracked for `file`/`line` within `mod_id`,
    /// clearing the armed INT3 in `debugger` if one was set.
    pub fn clear(&mut self, debugger: &mut Debugger, mod_id: &str, file: &str, line: u32) -> Result<(), DebugError> {
        let mut remaining = Vec::with_capacity(self.breakpoints.len());
        for bp in self.breakpoints.drain(..) {
            if bp.mod_id == mod_id && bp.file == file && bp.line == line {
                if let Some(addr) = bp.addr {
                    debugger.clear_breakpoint(addr)?;
                }
            } else {
                remaining.push(bp);
            }
        }
        self.breakpoints = remaining;
        Ok(())
    }

    /// Call after a `yog-hot-reload` `HotReloader::reload` for `mod_id`
    /// succeeds: re-resolves every breakpoint tracked against that mod
    /// (regardless of which prior generation it was armed under) against
    /// `new_symbols`/`new_base`, clearing the stale raw address first.
    pub fn rebind_after_reload(
        &mut self,
        debugger: &mut Debugger,
        mod_id: &str,
        new_generation: ModuleGeneration,
        new_base: u64,
        new_symbols: &SymbolTable,
    ) -> Result<(), DebugError> {
        for bp in self.breakpoints.iter_mut().filter(|bp| bp.mod_id == mod_id) {
            if let Some(old_addr) = bp.addr.take() {
                debugger.clear_breakpoint(old_addr)?;
            }
            let new_addr = new_symbols.resolve_breakpoint(&bp.file, bp.line).into_iter().next().map(|offset| new_base + offset);
            if let Some(addr) = new_addr {
                debugger.set_breakpoint(addr)?;
            }
            bp.addr = new_addr;
            bp.generation = new_generation;
        }
        Ok(())
    }
}
