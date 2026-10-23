//! `yog designer-lib`: the mod's native for the IDE's designers.

pub(super) const WORDS: &[&str] = &["designer-lib"];

pub(super) fn run(_rest: &[String]) -> Result<(), String> {
    crate::cli::pipeline::designer_lib()
}
