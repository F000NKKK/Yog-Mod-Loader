//! `yog bench [args]`: the mod's `benches/*.rs` under criterion.

pub(super) const WORDS: &[&str] = &["bench"];

pub(super) fn run(rest: &[String]) -> Result<(), String> {
    crate::cli::pipeline::run_benches(rest)
}
