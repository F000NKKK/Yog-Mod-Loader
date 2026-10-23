//! `yog help`: what `yog` does (`usage.txt`).

pub(super) const WORDS: &[&str] = &["help", "-h", "--help"];
const USAGE: &str = include_str!("../usage.txt");

pub(super) fn run(_rest: &[String]) -> Result<(), String> {
    print!("{USAGE}");
    Ok(())
}
