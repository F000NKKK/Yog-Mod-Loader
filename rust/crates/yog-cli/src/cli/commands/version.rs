//! `yog version`: this CLI's version.

pub(super) const WORDS: &[&str] = &["version", "-V", "--version"];

pub(super) fn run(_rest: &[String]) -> Result<(), String> {
    println!("yog {}", env!("CARGO_PKG_VERSION"));
    Ok(())
}
