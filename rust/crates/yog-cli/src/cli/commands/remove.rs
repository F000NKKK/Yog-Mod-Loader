//! `yog remove <crate> [--mod]`: a dependency removed.

use crate::cli::flags::{MOD, has};

pub(super) const WORDS: &[&str] = &["remove"];

pub(super) fn run(rest: &[String]) -> Result<(), String> {
    crate::cli::project::deps::remove_dep(crate::cli::project::deps::dep_name(rest), has(rest, MOD))
}
