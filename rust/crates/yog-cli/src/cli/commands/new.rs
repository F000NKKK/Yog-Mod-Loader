//! `yog new <name>`: a new mod.

pub(super) const WORDS: &[&str] = &["new"];

pub(super) fn run(rest: &[String]) -> Result<(), String> {
    crate::cli::project::new::new_mod(rest.first().map(String::as_str))
}
