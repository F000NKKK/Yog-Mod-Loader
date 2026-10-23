//! The System library's declarations (`system/*.ys`), read once and seen
//! by every source as its own types are, namespace `System`.

use yog_sync::LazyLock;

use crate::ast::Decl;

/// The enum whose cases a source calls without its name (`Ok(v)`,
/// `Err(e)`), as Rust's prelude brings them.
pub const RESULT: &str = "Result";

/// The System library's sources, by file name.
const SOURCES: &[(&str, &str)] = &[("System.ys", include_str!("system/System.ys"))];

/// Every declaration of the library, parsed once.
pub static DECLS: LazyLock<Vec<Decl>> = LazyLock::new(|| {
    SOURCES
        .iter()
        .flat_map(|(name, src)| crate::parse(name, src).file.decls)
        .collect()
});
