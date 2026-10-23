//! The designers' host process (`yog_cli::design::host`).

pub(super) const WORDS: &[&str] = &[yog_cli::design::host::COMMAND];

pub(super) fn run(_rest: &[String]) -> Result<(), String> {
    yog_cli::design::host::serve();
    Ok(())
}
