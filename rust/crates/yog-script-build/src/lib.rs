//! Yog Script's compiler: a source checked (`check_source` — read, its
//! names resolved; with the generator, compiled), against the class index
//! of the Minecraft versions it targets (`index`, built from the game's
//! jars, read with `classfile`). It knows sources, not projects: the Yog
//! CLI hands it a mod's, the `yog-script` tool any.

mod classfile;
pub mod index;

use yog_data::Diagnostic;

/// What's wrong with the source `text` of the file `name` (`.ys`, `.yns`).
#[must_use]
pub fn check_source(name: &str, text: &str) -> Vec<Diagnostic> {
    yog_script_analyze::check(name, text).problems
}
