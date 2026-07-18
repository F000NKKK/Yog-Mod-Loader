//! `ptrace`-based native debugger for Yog mods, speaking a minimal subset
//! of the Debug Adapter Protocol (see [`dap`]) so Yog-IDLE (or any DAP
//! client) can drive it.
//!
//! Linux-only for this pass — `ptrace` is a Linux/BSD-specific syscall, and
//! `[cfg(target_os = "linux")]` keeps this crate buildable (with the
//! `ptrace_debugger`/`maps`/[`YogDebugSession`] pieces simply absent) on
//! other host platforms in the meantime; a real Windows (debug API) or
//! macOS (`task_for_pid`) backend is future work, not attempted here.

pub mod dap;
pub mod source_breakpoints;

#[cfg(target_os = "linux")]
pub mod maps;
#[cfg(target_os = "linux")]
pub mod ptrace_debugger;

#[cfg(target_os = "linux")]
pub use ptrace_debugger::{DebugError, Debugger, StopReason};
pub use source_breakpoints::{SourceBreakpoint, SourceBreakpoints};

#[cfg(target_os = "linux")]
mod session {
    use std::path::PathBuf;

    use serde_json::Value;
    use yog_hot_reload::ModuleGeneration;
    use yog_symbols::SymbolTable;

    use crate::dap::{DebugSession, StackFrameInfo};
    use crate::maps::find_module_base;
    use crate::ptrace_debugger::{Debugger, StopReason};
    use crate::source_breakpoints::SourceBreakpoints;

    /// A single-mod debug session: attaches to one running process and
    /// debugs one mod's native within it. Multi-mod attach (breakpoints
    /// spanning several mods loaded into the same process at once) isn't
    /// handled yet — `mod_id` is fixed at construction.
    pub struct YogDebugSession {
        mod_id: String,
        native_path: PathBuf,
        symbols: SymbolTable,
        generation: ModuleGeneration,
        debugger: Option<Debugger>,
        module_base: u64,
        breakpoints: SourceBreakpoints,
    }

    impl YogDebugSession {
        pub fn new(mod_id: impl Into<String>, native_path: PathBuf, generation: ModuleGeneration) -> Result<Self, yog_symbols::SymbolError> {
            let symbols = SymbolTable::load(&native_path)?;
            Ok(YogDebugSession {
                mod_id: mod_id.into(),
                native_path,
                symbols,
                generation,
                debugger: None,
                module_base: 0,
                breakpoints: SourceBreakpoints::new(),
            })
        }

        /// Call after a `yog-hot-reload` `HotReloader::reload` for this
        /// session's mod succeeds: reloads the new native's symbols and
        /// rebinds every tracked breakpoint against them.
        pub fn rebind_after_reload(&mut self, new_native_path: PathBuf, new_generation: ModuleGeneration) -> Result<(), String> {
            let debugger = self.debugger.as_mut().ok_or("not attached")?;
            let new_symbols = SymbolTable::load(&new_native_path).map_err(|e| e.to_string())?;
            let new_base = find_module_base(debugger.pid(), &new_native_path).ok_or("could not locate new native's load address")?;
            self.breakpoints
                .rebind_after_reload(debugger, &self.mod_id, new_generation, new_base, &new_symbols)
                .map_err(|e| e.to_string())?;
            self.native_path = new_native_path;
            self.symbols = new_symbols;
            self.generation = new_generation;
            self.module_base = new_base;
            Ok(())
        }

        fn resolve_stop_location(&self, addr: u64) -> Option<StackFrameInfo> {
            let offset = addr.checked_sub(self.module_base)?;
            let location = self.symbols.resolve_addr(offset)?;
            Some(StackFrameInfo {
                id: 0,
                name: location.function.unwrap_or_else(|| "<unknown>".to_string()),
                file: location.file.to_string_lossy().into_owned(),
                line: location.line,
                column: location.column.unwrap_or(0),
            })
        }
    }

    impl DebugSession for YogDebugSession {
        fn attach(&mut self, arguments: &Value) -> Result<(), String> {
            let pid = arguments.get("pid").and_then(Value::as_i64).ok_or("attach requires an integer \"pid\"")? as i32;
            let debugger = Debugger::attach(pid).map_err(|e| e.to_string())?;
            self.module_base = find_module_base(debugger.pid(), &self.native_path).ok_or("could not locate the mod's native in the target process's memory map — has it loaded yet?")?;
            self.debugger = Some(debugger);
            Ok(())
        }

        fn set_breakpoints(&mut self, source_path: &str, lines: &[u32]) -> Result<Vec<u32>, String> {
            let debugger = self.debugger.as_mut().ok_or("not attached")?;
            // Clear every breakpoint currently tracked for this file, then
            // re-add exactly the requested lines — matches DAP's
            // `setBreakpoints` "replace the whole set for this source"
            // semantics.
            let previous: Vec<u32> = debugger_lines_for(&self.breakpoints, source_path);
            for line in previous {
                self.breakpoints.clear(debugger, &self.mod_id, source_path, line).map_err(|e| e.to_string())?;
            }
            let mut verified = Vec::new();
            for &line in lines {
                self.breakpoints
                    .set(debugger, &self.mod_id, self.generation, self.module_base, &self.symbols, source_path, line)
                    .map_err(|e| e.to_string())?;
                verified.push(line);
            }
            Ok(verified)
        }

        fn continue_(&mut self) -> Result<(), String> {
            let debugger = self.debugger.as_mut().ok_or("not attached")?;
            match debugger.continue_().map_err(|e| e.to_string())? {
                StopReason::Exited(_) | StopReason::Killed(_) => Ok(()),
                StopReason::Breakpoint(_) | StopReason::Signal(_) => Ok(()),
            }
        }

        fn next(&mut self) -> Result<(), String> {
            let debugger = self.debugger.as_mut().ok_or("not attached")?;
            debugger.single_step().map_err(|e| e.to_string())?;
            Ok(())
        }

        fn step_in(&mut self) -> Result<(), String> {
            self.next()
        }

        fn stack_trace(&mut self) -> Result<Vec<StackFrameInfo>, String> {
            let debugger = self.debugger.as_ref().ok_or("not attached")?;
            let addrs = debugger.backtrace(64).map_err(|e| e.to_string())?;
            Ok(addrs
                .into_iter()
                .enumerate()
                .filter_map(|(i, addr)| {
                    let mut frame = self.resolve_stop_location(addr)?;
                    frame.id = i as i64;
                    Some(frame)
                })
                .collect())
        }

        fn threads(&self) -> Vec<(i64, String)> {
            match &self.debugger {
                Some(debugger) => vec![(debugger.pid().as_raw() as i64, self.mod_id.clone())],
                None => Vec::new(),
            }
        }

        fn disconnect(&mut self) -> Result<(), String> {
            if let Some(mut debugger) = self.debugger.take() {
                debugger.detach().map_err(|e| e.to_string())?;
            }
            Ok(())
        }
    }

    fn debugger_lines_for(breakpoints: &SourceBreakpoints, source_path: &str) -> Vec<u32> {
        breakpoints.all().iter().filter(|bp| bp.file == source_path).map(|bp| bp.line).collect()
    }
}

#[cfg(target_os = "linux")]
pub use session::YogDebugSession;
