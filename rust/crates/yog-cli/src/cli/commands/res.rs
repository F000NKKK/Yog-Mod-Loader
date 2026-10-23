//! `yog res check`: the mod's resources checked, without building.

pub(super) const WORDS: &[&str] = &["res"];
const CHECK: &str = "check";
const USAGE: &str = "usage: yog res check";

pub(super) fn run(rest: &[String]) -> Result<(), String> {
    match rest.first().map(String::as_str) {
        Some(CHECK) => crate::cli::pipeline::steps::res_check(),
        _ => Err(USAGE.into()),
    }
}
