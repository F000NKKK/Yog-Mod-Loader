//! The System library's declarations (`system/**/*.ys`: one type a file,
//! its folders its namespace), read once and seen by every source as its
//! own types are. Its names are `yog_script_ids::system`'s.

use yog_sync::LazyLock;

use crate::ast::Decl;

// `SOURCES`: every `.ys` under `src/system/`, listed by `build.rs`.
include!(concat!(env!("OUT_DIR"), "/system_sources.rs"));

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
