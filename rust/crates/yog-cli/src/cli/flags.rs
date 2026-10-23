//! The flags commands take.

pub(super) const DEV: &str = "--dev";
pub(super) const DEBUG: &str = "--debug";
pub(super) const LIST: &str = "--list";
pub(crate) const MOD: &str = "--mod";
pub(super) const DRY_RUN: &str = "--dry-run";
pub(super) const HELP: [&str; 2] = ["-h", "--help"];
/// What a flag starts with.
pub(super) const FLAG: &str = "--";

/// Whether `rest` has `flag`.
pub(super) fn has(rest: &[String], flag: &str) -> bool {
    rest.iter().any(|a| a == flag)
}

/// The words of `rest` that aren't flags.
pub(super) fn words(rest: &[String]) -> Vec<&str> {
    rest.iter()
        .map(String::as_str)
        .filter(|a| !a.starts_with(FLAG))
        .collect()
}
