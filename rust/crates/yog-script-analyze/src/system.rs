//! The System library's declarations (`system/*.ys`), read once and seen
//! by every source as its own types are, namespace `System`.

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

/// The bus's attributes (`System.Yog.Native`): a call, an event, a method provided.
pub const SERVICE: &str = "Service";
pub const EVENT: &str = "Event";
pub const PROVIDE: &str = "Provide";

/// The System library's sources, by file name.
const SOURCES: &[(&str, &str)] = &[
    ("System.ys", include_str!("system/System.ys")),
    ("System.Meta.ys", include_str!("system/System.Meta.ys")),
    (
        "System.Yog.Native.ys",
        include_str!("system/System.Yog.Native.ys"),
    ),
];

/// Every declaration of the library, parsed once.
pub static DECLS: LazyLock<Vec<Decl>> = LazyLock::new(|| {
    SOURCES
        .iter()
        .flat_map(|(name, src)| crate::parse(name, src).file.decls)
        .collect()
});
