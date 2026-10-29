//! DWARF-backed symbol resolution for a mod's compiled native library.
//!
//! Loads the *unstripped* cdylib a mod was built with (`yog build debug` /
//! `--debugging-symbols` keeps this around instead of only the stripped
//! release native) and answers the two questions the rest of the debugging
//! stack needs:
//!
//! - "what source line does this runtime address belong to" —
//!   [`SymbolTable::resolve_addr`], used for backtraces.
//! - "what address(es) does this source line compile to" —
//!   [`SymbolTable::resolve_breakpoint`], used to turn an IDE's file:line
//!   breakpoint request into something `yog-debugger` can `ptrace` onto.
//!
//! - "what locals and parameters are visible here, and what are their
//!   values" — [`SymbolTable::locals`] / [`SymbolTable::read_local`], for a
//!   debugger's Variables view. Registers and memory are supplied by the
//!   caller ([`FrameRegs`], [`TargetMemory`]); nothing here uses `ptrace`.
//!
//! - "what does this expression evaluate to" — [`SymbolTable::evaluate`]
//!   for Watch windows and DataTips, with [`SymbolTable::local_nodes`] /
//!   [`SymbolTable::expand`] for views that expand values one level at a
//!   time.
//!
//! - "who called this frame" — [`SymbolTable::unwind`] /
//!   [`SymbolTable::backtrace`], from the native's call frame info, so
//!   stacks walk through the frame-pointer-less standard library.
//!
//! Deliberately built on `gimli`/`object`/`addr2line` directly rather than a
//! single higher-level helper: `addr2line`'s own convenience API only goes
//! address -> line, and breakpoint placement needs the reverse (line ->
//! address), which means walking the DWARF line program ourselves anyway.

mod unwinding;
mod values;

pub use unwinding::arch;

use std::path::{Path, PathBuf};
use std::sync::{Arc, OnceLock};

use gimli::{EndianArcSlice, Reader as _, RunTimeEndian};
use object::{Object, ObjectSection, ObjectSegment, ObjectSymbol, SymbolKind};

pub use unwinding::arch::{Arch, aarch64, x86_64};
pub use values::eval::{EvalError, ValueNode, ValueRef};
pub use values::locals::{FrameRegs, LocalVar, TargetMemory, Value, VarKind};

// `Arc`, not `Rc`: a `SymbolTable` needs to be `Send + Sync` so host apps
// (e.g. Yog-IDLE's Tauri backend) can hold one behind a `Mutex` in shared
// state without fighting the type system — this is a read-mostly, rarely
// cloned reader, so atomic refcounting costs nothing that matters here.
type R = EndianArcSlice<RunTimeEndian>;

/// Most locations [`SymbolTable::resolve_inlined`] reports for one address.
const MAX_INLINE_DEPTH: usize = 64;

#[derive(Debug)]
pub enum SymbolError {
    Io(std::io::Error),
    Object(object::Error),
    Dwarf(gimli::Error),
}

impl std::fmt::Display for SymbolError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SymbolError::Io(e) => write!(f, "reading native: {e}"),
            SymbolError::Object(e) => write!(f, "parsing object file: {e}"),
            SymbolError::Dwarf(e) => write!(f, "parsing DWARF: {e}"),
        }
    }
}

impl std::error::Error for SymbolError {}

impl From<std::io::Error> for SymbolError {
    fn from(e: std::io::Error) -> Self {
        SymbolError::Io(e)
    }
}

impl From<object::Error> for SymbolError {
    fn from(e: object::Error) -> Self {
        SymbolError::Object(e)
    }
}

impl From<gimli::Error> for SymbolError {
    fn from(e: gimli::Error) -> Self {
        SymbolError::Dwarf(e)
    }
}

/// Where a runtime address maps to in source.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceLocation {
    pub file: PathBuf,
    pub line: u32,
    pub column: Option<u32>,
    /// The innermost function containing the address, demangled where possible.
    pub function: Option<String>,
}

/// One exported/global function symbol from the native's own symbol table
/// (not DWARF — this comes from the object file's symtab), for a future
/// symbol browser and for hot-reload's old/new function-set comparisons.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FunctionSymbol {
    pub name: String,
    pub address: u64,
}

/// A loaded native's debug info — resolves addresses to source locations
/// and source lines to candidate breakpoint addresses.
pub struct SymbolTable {
    dwarf: gimli::Dwarf<R>,
    ctx: addr2line::Context<R>,
    functions: Vec<FunctionSymbol>,
    /// Parsed compilation units, built on the first locals lookup.
    units: OnceLock<Vec<values::locals::UnitInfo>>,
    /// Statics by qualified name, built on the first watch expression
    /// that isn't a local.
    globals: OnceLock<Vec<values::eval::GlobalVar>>,
    /// Sorted addresses where a line-table row with `is_stmt` starts, built
    /// on the first [`Self::is_statement_start`].
    statements: OnceLock<Vec<u64>>,
    /// Call frame info for `DW_OP_call_frame_cfa` frame bases.
    eh_frame: Option<(gimli::EhFrame<R>, gimli::BaseAddresses)>,
    debug_frame: Option<gimli::DebugFrame<R>>,
    /// Address ranges of the executable sections (module-relative).
    code: Vec<(u64, u64)>,
    /// The address the file is linked to load at (a PE's or a Mach-O
    /// executable's image base; 0 for an ELF shared object): its DWARF
    /// addresses count from there.
    image_base: u64,
    /// A Windows native's exception tables (it has no `.eh_frame`).
    pe_unwind: Option<unwinding::pe_unwind::PeUnwind>,
}

/// The native's named code symbols.
fn function_symbols(object: &object::File<'_>) -> Vec<FunctionSymbol> {
    object
        .symbols()
        .filter(|s| s.kind() == SymbolKind::Text && !s.is_undefined())
        .filter_map(|s| {
            let name = s.name().ok()?.to_string();
            (!name.is_empty()).then_some(FunctionSymbol {
                name,
                address: s.address(),
            })
        })
        .collect()
}

/// Bytes of an address, on a 64-bit native and on a 32-bit one.
const ADDRESS_64: u8 = 8;
const ADDRESS_32: u8 = 4;
/// The section a native with debug info has.
const DEBUG_INFO: &str = ".debug_info";
/// A native's split debug info's extension (`libmod.so.debug`).
const SPLIT_EXT: &str = yog_ids::symbols::EXT;
/// Where macOS keeps a native's debug info apart from it (what `dsymutil`
/// makes — rustc's default there): `<native>.dSYM/Contents/Resources/DWARF/<native's name>`.
const DSYM_EXT: &str = "dSYM";
const DSYM_DWARF: [&str; 3] = ["Contents", "Resources", "DWARF"];

/// The address a native was linked to load its first byte (its header) at
/// — what the system reports a loaded module's base as. A Mach-O's is its
/// `__TEXT` segment's (`0x100000000` for an executable), which `object`
/// doesn't count addresses from; the others' is what it does.
fn link_base(object: &object::File<'_>) -> u64 {
    if object.format() != object::BinaryFormat::MachO {
        return object.relative_address_base();
    }
    object
        .segments()
        .find(|s| matches!(s.file_range(), (0, size) if size > 0))
        .map_or(0, |s| s.address())
}

/// The files a native's debug info may have been split into, beside it.
fn split_files(path: &Path) -> Vec<PathBuf> {
    let mut dsym = path.with_added_extension(DSYM_EXT);
    dsym.extend(DSYM_DWARF);
    let dsym = path.file_name().map(|name| dsym.join(name));
    std::iter::once(path.with_added_extension(SPLIT_EXT))
        .chain(dsym)
        .collect()
}

/// The unwind tables: `.eh_frame` (with the bases its pointers are
/// relative to) and `.debug_frame`, those the native has.
type FrameTables = (
    Option<(gimli::EhFrame<R>, gimli::BaseAddresses)>,
    Option<gimli::DebugFrame<R>>,
);

fn frame_tables(object: &object::File<'_>, endian: RunTimeEndian) -> FrameTables {
    let address_size = if object.is_64() {
        ADDRESS_64
    } else {
        ADDRESS_32
    };
    let section_reader = |name: &str| -> Option<(R, u64)> {
        let section = object.section_by_name(name)?;
        let data = section.uncompressed_data().ok()?.into_owned();
        if data.is_empty() {
            return None;
        }
        Some((
            EndianArcSlice::new(Arc::from(data.into_boxed_slice()), endian),
            section.address(),
        ))
    };
    let eh_frame = section_reader(".eh_frame").map(|(data, addr)| {
        let mut eh_frame = gimli::EhFrame::from(data);
        eh_frame.set_address_size(address_size);
        let mut bases = gimli::BaseAddresses::default().set_eh_frame(addr);
        if let Some(text) = object.section_by_name(".text") {
            bases = bases.set_text(text.address());
        }
        if let Some(got) = object.section_by_name(".got") {
            bases = bases.set_got(got.address());
        }
        (eh_frame, bases)
    });
    let debug_frame = section_reader(".debug_frame").map(|(data, _)| {
        let mut debug_frame = gimli::DebugFrame::from(data);
        debug_frame.set_address_size(address_size);
        debug_frame
    });
    (eh_frame, debug_frame)
}

impl SymbolTable {
    /// What every address this table takes or gives (`offset`s) is relative
    /// to, for a module loaded at runtime address `base`: `base` less the
    /// address the file was linked to load at. Pass it wherever a
    /// `load_base` is asked for.
    #[must_use]
    pub fn load_bias(&self, base: u64) -> u64 {
        base.wrapping_sub(self.image_base)
    }

    /// Loads a native's debug info: its own (built with `debug = true`,
    /// `strip = false`), or — a release's, stripped — the file split from it
    /// beside it (`<native>.debug`, `yog build release`; macOS's `.dSYM`).
    pub fn load(path: &Path) -> Result<Self, SymbolError> {
        let data = std::fs::read(path)?;
        let own = object::File::parse(&*data)?
            .section_by_name(DEBUG_INFO)
            .is_some();
        if !own && let Some(beside) = split_files(path).into_iter().find(|f| f.is_file()) {
            return Self::load_split(path, &beside);
        }
        Self::load_split(path, path)
    }

    /// Loads `native`'s code and unwind tables with the debug info in
    /// `debug` (the same file, or the one split from it).
    pub fn load_split(native: &Path, debug: &Path) -> Result<Self, SymbolError> {
        let data = std::fs::read(native)?;
        let object = object::File::parse(&*data)?;
        let debug_data = if debug == native {
            None
        } else {
            Some(std::fs::read(debug)?)
        };
        let debug_object = match &debug_data {
            Some(d) => Some(object::File::parse(&**d)?),
            None => None,
        };
        let dwarf_source = debug_object.as_ref().unwrap_or(&object);

        let endian = if object.is_little_endian() {
            RunTimeEndian::Little
        } else {
            RunTimeEndian::Big
        };

        let load_section = |id: gimli::SectionId| -> Result<R, SymbolError> {
            let data = match dwarf_source.section_by_name(id.name()) {
                Some(section) => section.uncompressed_data()?.into_owned(),
                None => Vec::new(),
            };
            Ok(EndianArcSlice::new(
                Arc::from(data.into_boxed_slice()),
                endian,
            ))
        };

        // `gimli::Dwarf<R>` isn't `Clone` here, and `addr2line::Context`
        // takes ownership of the one it's given — load the sections twice
        // (cheap: a handful of small `Arc<[u8]>` allocations) rather than
        // fight that, so `SymbolTable` still keeps its own `Dwarf` for the
        // breakpoint line-program walk in `resolve_breakpoint`.
        let dwarf = gimli::Dwarf::load(load_section)?;
        let ctx = addr2line::Context::from_dwarf(gimli::Dwarf::load(load_section)?)?;

        let functions = function_symbols(&object);
        let (eh_frame, debug_frame) = frame_tables(&object, endian);
        let code = object
            .sections()
            .filter(|s| s.kind() == object::SectionKind::Text && s.size() > 0)
            .map(|s| (s.address(), s.address() + s.size()))
            .collect();

        let image_base = link_base(&object);
        Ok(SymbolTable {
            image_base,
            pe_unwind: unwinding::pe_unwind::PeUnwind::of(&object),
            dwarf,
            ctx,
            functions,
            units: OnceLock::new(),
            globals: OnceLock::new(),
            statements: OnceLock::new(),
            eh_frame,
            debug_frame,
            code,
        })
    }

    /// Resolves a runtime address (already translated to an offset within
    /// this native — callers dealing with a live process still need to
    /// subtract the module's load base first) to a source location.
    pub fn resolve_addr(&self, addr: u64) -> Option<SourceLocation> {
        let location = self.ctx.find_location(addr).ok().flatten()?;
        let function = self
            .ctx
            .find_frames(addr)
            .skip_all_loads()
            .ok()
            .and_then(|mut frames| function_name(frames.next().ok().flatten()?.function.as_ref()?));

        Some(SourceLocation {
            file: normalize_path(Path::new(location.file.unwrap_or(""))),
            line: location.line.unwrap_or(0),
            column: location.column,
            function,
        })
    }

    /// Like [`Self::resolve_addr`], but one location per inlined call at
    /// `addr`, innermost first: the inlined function's own line, then each
    /// caller's call site, ending with the out-of-line function. Lets a
    /// debugger treat `std` code inlined into user code as part of the
    /// user's line rather than as a separate place.
    pub fn resolve_inlined(&self, addr: u64) -> Vec<SourceLocation> {
        let mut out = Vec::new();
        let Ok(mut frames) = self.ctx.find_frames(addr).skip_all_loads() else {
            return out;
        };
        while let Ok(Some(frame)) = frames.next() {
            let location = frame.location.as_ref();
            out.push(SourceLocation {
                file: normalize_path(Path::new(location.and_then(|l| l.file).unwrap_or(""))),
                line: location.and_then(|l| l.line).unwrap_or(0),
                column: location.and_then(|l| l.column),
                function: frame.function.as_ref().and_then(function_name),
            });
            if out.len() >= MAX_INLINE_DEPTH {
                break;
            }
        }
        out
    }

    /// Every runtime address the DWARF line program associates with `line`
    /// in `file`. An absolute `file` (what an IDE passes) must be the same
    /// file — line-table paths are resolved against the unit's
    /// `DW_AT_comp_dir` and normalized first, since `yog build` compiles from
    /// `obj/` with sources at `../src/`. A relative `file` matches on
    /// trailing path components, so callers can pass just a filename or a
    /// partial path.
    pub fn resolve_breakpoint(&self, file: &str, line: u32) -> Vec<u64> {
        let want_file = normalize_path(Path::new(file));
        let want_name = want_file.file_name();
        let mut addrs = Vec::new();

        self.for_each_line_program(|unit, program| {
            let comp_dir = unit
                .comp_dir
                .as_ref()
                .and_then(|d| d.to_string_lossy().ok())
                .map(|d| PathBuf::from(d.as_ref()));
            // Match the unit's file table once, so units that never mention
            // the file (most of them: std, dependencies) skip their rows;
            // the bare file name is compared before building a full path.
            let header = program.header();
            let matches = |entry: &gimli::FileEntry<R>| {
                if let Some(want_name) = want_name {
                    let Ok(name) = self.dwarf.attr_string(unit, entry.path_name()) else {
                        return false;
                    };
                    let Ok(name) = name.to_string_lossy() else {
                        return false;
                    };
                    if Path::new(name.as_ref()).file_name() != Some(want_name) {
                        return false;
                    }
                }
                self.line_file_path(unit, header, entry, comp_dir.as_deref())
                    .is_some_and(|row_file| {
                        if want_file.is_absolute() {
                            row_file == want_file
                        } else {
                            row_file.ends_with(&want_file)
                        }
                    })
            };
            let matching: Vec<bool> = (0..=header.file_names().len() as u64)
                .map(|index| header.file(index).is_some_and(matches))
                .collect();
            if !matching.contains(&true) {
                return;
            }
            let mut rows = program.rows();
            while let Ok(Some((_, row))) = rows.next_row() {
                // The end-of-sequence row carries the last line but its
                // address is one past the code (often the next function).
                if row.end_sequence() || row.line().map(|l| l.get()) != Some(u64::from(line)) {
                    continue;
                }
                let file_index = usize::try_from(row.file_index()).unwrap_or(usize::MAX);
                if matching.get(file_index).copied().unwrap_or(false) {
                    addrs.push(row.address());
                }
            }
        });

        addrs.sort_unstable();
        addrs.dedup();
        addrs
    }

    /// Runs `f` on the line program of every unit that has one.
    fn for_each_line_program(
        &self,
        mut f: impl FnMut(&gimli::Unit<R>, gimli::IncompleteLineProgram<R>),
    ) {
        let mut units = self.dwarf.units();
        while let Ok(Some(header)) = units.next() {
            let Ok(unit) = self.dwarf.unit(header) else {
                continue;
            };
            if let Some(program) = unit.line_program.clone() {
                f(&unit, program);
            }
        }
    }

    /// A line-table file entry's full, normalized path: `comp_dir` /
    /// include directory / file name (a later absolute part replaces what
    /// came before it, as `PathBuf::push` does).
    fn line_file_path(
        &self,
        unit: &gimli::Unit<R>,
        header: &gimli::LineProgramHeader<R>,
        file_entry: &gimli::FileEntry<R>,
        comp_dir: Option<&Path>,
    ) -> Option<PathBuf> {
        let mut path = comp_dir.map(Path::to_path_buf).unwrap_or_default();
        if let Some(dir) = file_entry.directory(header) {
            let dir = self.dwarf.attr_string(unit, dir).ok()?;
            path.push(dir.to_string_lossy().ok()?.as_ref());
        }
        let name = self.dwarf.attr_string(unit, file_entry.path_name()).ok()?;
        path.push(name.to_string_lossy().ok()?.as_ref());
        Some(normalize_path(&path))
    }

    /// Whether `addr` (an offset within this native) is where a statement
    /// begins — a line-table row marked `is_stmt`, which is where a debugger
    /// should stop a source-level step. Rows without it are the middle of a
    /// statement the compiler interleaved with another; stopping there is
    /// what makes optimized code's steps bounce between lines.
    pub fn is_statement_start(&self, addr: u64) -> bool {
        let statements = self.statements.get_or_init(|| {
            let mut addrs = Vec::new();
            self.for_each_line_program(|_, program| {
                let mut rows = program.rows();
                while let Ok(Some((_, row))) = rows.next_row() {
                    if row.is_stmt() && !row.end_sequence() {
                        addrs.push(row.address());
                    }
                }
            });
            addrs.sort_unstable();
            addrs.dedup();
            addrs
        });
        statements.binary_search(&addr).is_ok()
    }

    /// Every global function symbol in the native's own symbol table.
    pub fn functions(&self) -> impl Iterator<Item = &FunctionSymbol> {
        self.functions.iter()
    }
}

/// A frame's function name, demangled where possible.
fn function_name(function: &addr2line::FunctionName<R>) -> Option<String> {
    match function.demangle() {
        Ok(demangled) => Some(demangled.into_owned()),
        Err(_) => Some(function.raw_name().ok()?.into_owned()),
    }
}

/// Lexically resolves `.` and `..` — DWARF paths like
/// `<mod>/obj/../src/lib.rs` must compare equal to the
/// `<mod>/src/lib.rs` an editor has open. No filesystem access (the paths
/// may not exist on this machine).
fn normalize_path(path: &Path) -> PathBuf {
    use std::path::Component;
    let mut out = PathBuf::new();
    for component in path.components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir => {
                if !out.pop() {
                    out.push("..");
                }
            }
            other => out.push(other.as_os_str()),
        }
    }
    out
}
