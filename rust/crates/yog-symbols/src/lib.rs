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
//! Deliberately built on `gimli`/`object`/`addr2line` directly rather than a
//! single higher-level helper: `addr2line`'s own convenience API only goes
//! address -> line, and breakpoint placement needs the reverse (line ->
//! address), which means walking the DWARF line program ourselves anyway.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use gimli::{EndianArcSlice, Reader as _, RunTimeEndian};
use object::{Object, ObjectSection, ObjectSymbol, SymbolKind};

// `Arc`, not `Rc`: a `SymbolTable` needs to be `Send + Sync` so host apps
// (e.g. Yog-IDLE's Tauri backend) can hold one behind a `Mutex` in shared
// state without fighting the type system — this is a read-mostly, rarely
// cloned reader, so atomic refcounting costs nothing that matters here.
type R = EndianArcSlice<RunTimeEndian>;

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
}

impl SymbolTable {
    /// Loads debug info from an unstripped native (`.so`/`.dylib`/`.dll`
    /// built with `debug = true`, `strip = false`).
    pub fn load(path: &Path) -> Result<Self, SymbolError> {
        let data = std::fs::read(path)?;
        let object = object::File::parse(&*data)?;

        let endian = if object.is_little_endian() {
            RunTimeEndian::Little
        } else {
            RunTimeEndian::Big
        };

        let load_section = |id: gimli::SectionId| -> Result<R, SymbolError> {
            let data = match object.section_by_name(id.name()) {
                Some(section) => section.uncompressed_data()?.into_owned(),
                None => Vec::new(),
            };
            Ok(EndianArcSlice::new(Arc::from(data.into_boxed_slice()), endian))
        };

        // `gimli::Dwarf<R>` isn't `Clone` here, and `addr2line::Context`
        // takes ownership of the one it's given — load the sections twice
        // (cheap: a handful of small `Arc<[u8]>` allocations) rather than
        // fight that, so `SymbolTable` still keeps its own `Dwarf` for the
        // breakpoint line-program walk in `resolve_breakpoint`.
        let dwarf = gimli::Dwarf::load(load_section)?;
        let ctx = addr2line::Context::from_dwarf(gimli::Dwarf::load(load_section)?)?;

        let functions = object
            .symbols()
            .filter(|s| s.kind() == SymbolKind::Text && !s.is_undefined())
            .filter_map(|s| {
                let name = s.name().ok()?.to_string();
                if name.is_empty() {
                    return None;
                }
                Some(FunctionSymbol { name, address: s.address() })
            })
            .collect();

        Ok(SymbolTable { dwarf, ctx, functions })
    }

    /// Resolves a runtime address (already translated to an offset within
    /// this native — callers dealing with a live process still need to
    /// subtract the module's load base first) to a source location.
    pub fn resolve_addr(&self, addr: u64) -> Option<SourceLocation> {
        let location = self.ctx.find_location(addr).ok().flatten()?;
        let function = self.ctx.find_frames(addr).skip_all_loads().ok().and_then(|mut frames| {
            let frame = frames.next().ok().flatten()?;
            let f = frame.function?;
            let name = match f.demangle() {
                Ok(demangled) => demangled.into_owned(),
                Err(_) => f.raw_name().ok()?.into_owned(),
            };
            Some(name)
        });

        Some(SourceLocation {
            file: PathBuf::from(location.file.unwrap_or("")),
            line: location.line.unwrap_or(0),
            column: location.column,
            function,
        })
    }

    /// Every runtime address the DWARF line program associates with `line`
    /// in a file whose path ends with `file` (matched on trailing path
    /// components so callers can pass just a filename or a partial path).
    pub fn resolve_breakpoint(&self, file: &str, line: u32) -> Vec<u64> {
        let want_file = Path::new(file);
        let mut addrs = Vec::new();

        let mut units = self.dwarf.units();
        while let Ok(Some(header)) = units.next() {
            let Ok(unit) = self.dwarf.unit(header) else { continue };
            let Some(program) = unit.line_program.clone() else { continue };
            let mut rows = program.rows();
            while let Ok(Some((line_header, row))) = rows.next_row() {
                let Some(row_line) = row.line() else { continue };
                if row_line.get() as u32 != line {
                    continue;
                }
                let Some(file_entry) = row.file(line_header) else { continue };
                let Ok(path_name) = self.dwarf.attr_string(&unit, file_entry.path_name()) else { continue };
                let Ok(path_str) = path_name.to_string_lossy() else { continue };
                if Path::new(path_str.as_ref()).ends_with(want_file) {
                    addrs.push(row.address());
                }
            }
        }

        addrs.sort_unstable();
        addrs.dedup();
        addrs
    }

    /// Every global function symbol in the native's own symbol table.
    pub fn functions(&self) -> impl Iterator<Item = &FunctionSymbol> {
        self.functions.iter()
    }
}
