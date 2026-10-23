//! `yog content check`: the mod's blocks and items checked, without building.

pub(super) const WORDS: &[&str] = &["content"];
const CHECK: &str = "check";
const USAGE: &str = "usage: yog content check";

pub(super) fn run(rest: &[String]) -> Result<(), String> {
    match rest.first().map(String::as_str) {
        Some(CHECK) => crate::cli::build::steps::content_check(),
        _ => Err(USAGE.into()),
    }
}
