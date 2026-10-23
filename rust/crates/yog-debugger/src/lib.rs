//! Native debugger for Yog mods — breakpoints, source steps, threads,
//! locals — on Linux (`ptrace`), Windows (the debug API) and macOS (Mach
//! exception ports), x86-64 and AArch64; speaking a minimal subset of the
//! Debug Adapter Protocol (see [`dap`]) so Yog-IDLE (or any DAP client) can
//! drive it. [`target`] is what each system's backend offers, [`Debugger`]
//! the debugging built on it.

mod arch;
/// Writes a backend's calls and events to stderr with the `event-log`
/// feature; nothing without it.
macro_rules! event_log {
    ($($arg:tt)*) => {
        #[cfg(feature = "event-log")]
        eprintln!("[yog-debugger] {}", format_args!($($arg)*));
    };
}

pub mod dap;
mod debugger;
#[cfg(target_os = "linux")]
pub mod discovery;
mod platform;
pub mod source_breakpoints;
pub mod target;

pub use debugger::{DebugError, Debugger, StepKind, StopReason};
pub use platform::{interrupt, process_ended};
pub use source_breakpoints::{LoadedMod, SourceBreakpoint, SourceBreakpoints};
pub use target::{Module, Regs, Tid};

use std::path::Path;

use yog_symbols::{SourceLocation, SymbolTable};

/// Whether `path` (a DWARF source path) belongs to the Rust toolchain or a
/// dependency rather than the user's own code: `std` (`/rustc/<commit>/...`
/// or an installed `rust-src`) and crates from the cargo registry or git.
/// Stepping skips such code, like Visual Studio's Just My Code.
pub fn is_library_source(path: &Path) -> bool {
    let path = path.to_string_lossy().replace('\\', "/");
    path.starts_with("/rustc/")
        || [
            "/.cargo/registry/",
            "/.cargo/git/",
            "/.rustup/",
            "/lib/rustlib/src/",
        ]
        .iter()
        .any(|dir| path.contains(dir))
}

/// The innermost location at `offset` (module-relative) that is user code
/// — has a line and isn't [`is_library_source`] — looking through inlined
/// calls, so `std` code inlined into a user function counts as that
/// function's line. `None` in library code or code without lines.
pub fn user_location(symbols: &SymbolTable, offset: u64) -> Option<SourceLocation> {
    symbols
        .resolve_inlined(offset)
        .into_iter()
        .find(|loc| loc.line != 0 && !is_library_source(&loc.file))
}

mod session {
    use std::path::PathBuf;

    use serde_json::Value;
    use yog_hot_reload::ModuleGeneration;
    use yog_symbols::SymbolTable;

    use crate::Debugger;
    use crate::dap::{DebugSession, StackFrameInfo};
    use crate::source_breakpoints::{LoadedMod, SourceBreakpoints};

    const NOT_ATTACHED: &str = "not attached";
    /// How many frames a DAP `stackTrace` unwinds at most.
    const MAX_STACK_FRAMES: usize = 64;

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
        pub fn new(
            mod_id: impl Into<String>,
            native_path: PathBuf,
            generation: ModuleGeneration,
        ) -> Result<Self, yog_symbols::SymbolError> {
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
        pub fn rebind_after_reload(
            &mut self,
            new_native_path: PathBuf,
            new_generation: ModuleGeneration,
        ) -> Result<(), String> {
            let debugger = self.debugger.as_mut().ok_or(NOT_ATTACHED)?;
            let new_symbols = SymbolTable::load(&new_native_path).map_err(|e| e.to_string())?;
            let new_base = debugger
                .module_base(&new_native_path)
                .map(|base| new_symbols.load_bias(base))
                .ok_or("could not locate new native's load address")?;
            let m = LoadedMod {
                id: &self.mod_id,
                generation: new_generation,
                base: new_base,
                symbols: &new_symbols,
            };
            self.breakpoints
                .rebind_after_reload(debugger, &m)
                .map_err(|e| e.to_string())?;
            self.native_path = new_native_path;
            self.symbols = new_symbols;
            self.generation = new_generation;
            self.module_base = new_base;
            Ok(())
        }

        fn debugger(&mut self) -> Result<&mut Debugger, String> {
            self.debugger
                .as_mut()
                .ok_or_else(|| NOT_ATTACHED.to_string())
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
            let pid = arguments
                .get("pid")
                .and_then(Value::as_i64)
                .ok_or("attach requires an integer \"pid\"")? as u32;
            let debugger = Debugger::attach(pid).map_err(|e| e.to_string())?;
            self.module_base = debugger.module_base(&self.native_path).map(|base| self.symbols.load_bias(base)).ok_or("could not locate the mod's native in the target process's memory map — has it loaded yet?")?;
            self.debugger = Some(debugger);
            Ok(())
        }

        fn set_breakpoints(
            &mut self,
            source_path: &str,
            lines: &[u32],
        ) -> Result<Vec<u32>, String> {
            let debugger = self.debugger.as_mut().ok_or(NOT_ATTACHED)?;
            // Clear every breakpoint currently tracked for this file, then
            // re-add exactly the requested lines — matches DAP's
            // `setBreakpoints` "replace the whole set for this source"
            // semantics.
            let previous: Vec<u32> = debugger_lines_for(&self.breakpoints, source_path);
            for line in previous {
                self.breakpoints
                    .clear(debugger, &self.mod_id, source_path, line)
                    .map_err(|e| e.to_string())?;
            }
            let mut verified = Vec::new();
            let m = LoadedMod {
                id: &self.mod_id,
                generation: self.generation,
                base: self.module_base,
                symbols: &self.symbols,
            };
            for &line in lines {
                self.breakpoints
                    .set(debugger, &m, source_path, line)
                    .map_err(|e| e.to_string())?;
                verified.push(line);
            }
            Ok(verified)
        }

        fn continue_(&mut self) -> Result<(), String> {
            // Every stop reason, the process ending included, is a plain
            // success at the DAP level.
            self.debugger()?.continue_().map_err(|e| e.to_string())?;
            Ok(())
        }

        fn next(&mut self) -> Result<(), String> {
            self.debugger()?.single_step().map_err(|e| e.to_string())?;
            Ok(())
        }

        fn step_in(&mut self) -> Result<(), String> {
            self.next()
        }

        fn stack_trace(&mut self) -> Result<Vec<StackFrameInfo>, String> {
            let debugger = self.debugger.as_ref().ok_or(NOT_ATTACHED)?;
            let frames = debugger
                .backtrace_with(&self.symbols, self.module_base, MAX_STACK_FRAMES)
                .map_err(|e| e.to_string())?;
            Ok(frames
                .into_iter()
                .enumerate()
                .filter_map(|(i, regs)| {
                    let mut frame = self.resolve_stop_location(regs.pc())?;
                    frame.id = i as i64;
                    Some(frame)
                })
                .collect())
        }

        fn threads(&self) -> Vec<(i64, String)> {
            match &self.debugger {
                Some(debugger) => vec![(i64::from(debugger.pid()), self.mod_id.clone())],
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
        breakpoints
            .all()
            .iter()
            .filter(|bp| bp.file == source_path)
            .map(|bp| bp.line)
            .collect()
    }
}

pub use session::YogDebugSession;
