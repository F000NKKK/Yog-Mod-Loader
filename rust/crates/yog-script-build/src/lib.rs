//! Yog Script's compiler: a source checked (`check_source` — read, its
//! names resolved, its types checked; with the generator, compiled). A
//! `.ys` reaches Yog only; a `.yns` reaches the game too, and is checked
//! on every version it targets against that version's class index
//! (`index`, built from the game's jars, read with `classfile`). It knows
//! sources, not projects: the Yog CLI hands it a mod's, the `yog-script`
//! tool any.

mod classfile;
pub mod index;
mod macros;
mod versions;

use yog_data::Diagnostic;

pub use macros::MacroLibrary;
pub use yog_script_analyze::versions::targeted;

/// What's wrong with the source `text` of the file `name` (`.ys`, `.yns`);
/// a `.yns` checked on each version it targets, its classes from the cache
/// (built the first time).
#[must_use]
pub fn check_source(name: &str, text: &str) -> Vec<Diagnostic> {
    check_source_with(name, text, index::load)
}

/// [`check_source`], each version's classes from `load`.
pub fn check_source_with(
    name: &str,
    text: &str,
    load: impl Fn(&str) -> Result<index::ClassIndex, String>,
) -> Vec<Diagnostic> {
    check_source_using(name, text, load, None)
}

/// [`check_source_with`], the build's procedural macros `macros` run where
/// the source calls them.
pub fn check_source_using(
    name: &str,
    text: &str,
    load: impl Fn(&str) -> Result<index::ClassIndex, String>,
    macros: Option<&MacroLibrary>,
) -> Vec<Diagnostic> {
    let macros = macros.map(|m| m as &dyn yog_script_analyze::macros::ProcMacros);
    if !yog_script_analyze::is_nanites(name) {
        let env = yog_script_analyze::Env { game: None, macros };
        return yog_script_analyze::check_in(name, text, env).problems;
    }
    versions::check(name, text, load, macros)
}
