//! `yog setup`: the toolchain a mod builds with, installed where missing.

pub(super) const WORDS: &[&str] = &["setup"];

pub(super) fn run(_rest: &[String]) -> Result<(), String> {
    crate::cli::project::setup::setup()
}
