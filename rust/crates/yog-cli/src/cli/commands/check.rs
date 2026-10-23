//! `yog check`: the mod checked — its sources generated and its scripts
//! read, then `cargo check` for this machine.

pub(super) const WORDS: &[&str] = &["check"];

pub(super) fn run(_rest: &[String]) -> Result<(), String> {
    crate::cli::pipeline::check()
}
