//! `yog add <crate> [--mod]`: a dependency added (a Yog mod's with `--mod`).

use crate::cli::flags::{MOD, has};

pub(super) const WORDS: &[&str] = &["add"];

pub(super) fn run(rest: &[String]) -> Result<(), String> {
    crate::cli::project::deps::add_dep(crate::cli::project::deps::dep_name(rest), has(rest, MOD))
}
