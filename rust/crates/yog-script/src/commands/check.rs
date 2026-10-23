//! `yog-script check <file>...`: each source checked — a `.yns` on every
//! game version it targets — what's wrong said; an error when one has errors.

pub(super) const WORDS: &[&str] = &["check"];
const USAGE: &str = "usage: yog-script check <file.ys|file.yns>...";

pub(super) fn run(rest: &[String]) -> Result<(), String> {
    if rest.is_empty() {
        return Err(USAGE.into());
    }
    let mut errors = 0;
    for file in rest {
        let text = std::fs::read_to_string(file).map_err(|e| format!("{file}: {e}"))?;
        for d in yog_script_build::check_source(file, &text) {
            errors += usize::from(d.is_error());
            eprintln!("{d}");
        }
    }
    match errors {
        0 => Ok(()),
        n => Err(format!("{n} error(s)")),
    }
}
