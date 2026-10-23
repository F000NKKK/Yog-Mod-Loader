//! `yog doctor`: what the machine has of what building a mod needs.

pub(super) const WORDS: &[&str] = &["doctor"];

pub(super) fn run(_rest: &[String]) -> Result<(), String> {
    crate::cli::project::setup::doctor_cmd();
    Ok(())
}
