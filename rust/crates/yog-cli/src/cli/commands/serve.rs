//! `yog serve`: the CLI as a service for the IDE (`yog_cli::serve`).

pub(super) const WORDS: &[&str] = &["serve"];

pub(super) fn run(_rest: &[String]) -> Result<(), String> {
    yog_cli::serve::run()
}
