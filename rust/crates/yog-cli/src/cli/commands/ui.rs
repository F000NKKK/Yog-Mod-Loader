//! `yog ui gen [file.yui ...]`: screens' code generated (no files: every
//! `src/**/*.yui`).

pub(super) const WORDS: &[&str] = &["ui"];
const GEN: &str = "gen";
const USAGE: &str = "usage: yog ui gen [file.yui ...]  (no files: every src/**/*.yui)";

pub(super) fn run(rest: &[String]) -> Result<(), String> {
    match rest.first().map(String::as_str) {
        Some(GEN) => crate::cli::pipeline::steps::ui_gen(rest.get(1..).unwrap_or_default()),
        _ => Err(USAGE.into()),
    }
}
