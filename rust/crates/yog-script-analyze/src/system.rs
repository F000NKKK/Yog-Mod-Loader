//! The System library's declarations (`system/**/*.ys`: one type a file,
//! its folders its namespace), read once and seen by every source as its
//! own types are.

use yog_sync::LazyLock;

use crate::ast::Decl;

/// Every namespace of the library and of what a source reaches starts
/// here: `System`, `System.Yog` (Yog's objects), `System.Game` (the game's
/// classes, a `.yns`'s).
pub const ROOT: &str = "System";

/// The enum whose cases a source calls without its name (`Ok(v)`,
/// `Err(e)`), as Rust's prelude brings them.
pub const RESULT: &str = "Result";

/// The attribute that runs derive macros on a type: `[Derive(Clone)]`.
pub const DERIVE: &str = "Derive";

/// What a derive macro is given and its fields (`System.Meta`).
pub const ITEM: &str = "Item";
pub const FIELD: &str = "Field";

/// The attributes marking macros' methods (`System.Meta`).
pub const PROC_MACRO: &str = "ProcMacro";
pub const DERIVE_MACRO: &str = "DeriveMacro";

/// The attribute marking an attribute class as an attribute macro (`System`).
pub const MACRO: &str = "Macro";

/// An indexer's member name (`x[i]`): C#'s `IndexerName` default, the
/// CLR's `get_Item` / `set_Item` — what inheritance and the JVM call it.
pub const INDEXER: &str = "Item";

/// The bus's attributes (`System.Yog.Native`): a call, an event, a method provided.
pub const SERVICE: &str = "Service";
pub const EVENT: &str = "Event";
pub const PROVIDE: &str = "Provide";

// `SOURCES`: every `.ys` under `src/system/`, listed by `build.rs`.
include!(concat!(env!("OUT_DIR"), "/system_sources.rs"));

/// What separates a source's folders (`System/Meta/Item.ys`).
pub const FOLDER_SEP: char = '/';

/// The library's sources: each one's path under `src/system/` and its text.
pub fn sources() -> &'static [(&'static str, &'static str)] {
    SOURCES
}

/// Every declaration of the library, parsed once.
pub static DECLS: LazyLock<Vec<Decl>> = LazyLock::new(|| {
    SOURCES
        .iter()
        .flat_map(|(name, src)| crate::parse(name, src).file.decls)
        .collect()
});
