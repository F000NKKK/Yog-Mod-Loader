//! `yog-script check <file>...`: each source checked — a `.yns` on every
//! game version it targets — what's wrong said; an error when one has errors.

pub(super) const WORDS: &[&str] = &["check"];
const USAGE: &str = "usage: yog-script check [--macros <file.ys>]... <file.ys|file.yns>...";

pub(super) fn run(rest: &[String]) -> Result<(), String> {
    let (macros, files) = super::macros::split(rest)?;
    if files.is_empty() {
        return Err(USAGE.into());
    }
    let mut errors = 0;
    for file in &files {
        let text = std::fs::read_to_string(file).map_err(|e| format!("{file}: {e}"))?;
        let load = yog_script_build::index::load;
        for d in yog_script_build::check_source_using(file, &text, load, macros.as_ref()) {
            errors += usize::from(d.is_error());
            eprintln!("{d}");
        }
    }
    match errors {
        0 => Ok(()),
        n => Err(format!("{n} error(s)")),
    }
}
