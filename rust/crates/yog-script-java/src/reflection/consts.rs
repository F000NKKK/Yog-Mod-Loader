//! What Reflection's descriptions are written with: their lambdas'
//! parameters (the object, a value set, the arguments, the description),
//! no flags, and flags joined.

use yog_script_ids::java::BIT_OR;

pub(super) const OBJECT: &str = "o";
pub(super) const VALUE: &str = "v";
pub(super) const ARGS: &str = "a";
pub(super) const DESCRIPTION: &str = "d";
/// No flags.
pub(super) const NO_TRAITS: &str = "0";

/// Flags joined (`Type.CLASS | Type.SEALED`).
pub(super) fn flags_sep() -> String {
    format!(" {BIT_OR} ")
}
