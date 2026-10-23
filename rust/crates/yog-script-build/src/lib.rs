//! Yog Script's compiler: a source checked (`check_source` — read, its
//! names resolved, its types checked; with the generator, compiled). A
//! `.ys` reaches Yog only; a `.yns` reaches the game too, and is checked
//! on every version it targets against that version's class index
//! (`index`, built from the game's jars, read with `classfile`). It knows
//! sources, not projects: the Yog CLI hands it a mod's, the `yog-script`
//! tool any.

mod classfile;
pub mod index;
mod versions;

use yog_data::Diagnostic;

pub use versions::targeted;

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
    if !yog_script_analyze::is_nanites(name) {
        return yog_script_analyze::check(name, text).problems;
    }
    versions::check(name, text, load)
}
