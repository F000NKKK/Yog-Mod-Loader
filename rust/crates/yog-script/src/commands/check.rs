//! `yog-script check [--mc <version>] <file>...`: each source checked —
//! against the game's classes of that version when it's given — what's
//! wrong said; an error when one has errors.

pub(super) const WORDS: &[&str] = &["check"];
const USAGE: &str = "usage: yog-script check [--mc <version>] <file.ys|file.yns>...";
/// The flag naming the game's version to check against.
const MC_FLAG: &str = "--mc";

pub(super) fn run(rest: &[String]) -> Result<(), String> {
    let (game, files) = match rest {
        [flag, mc, files @ ..] if flag == MC_FLAG => (Some(yog_script_build::index::load(mc)?), files),
        files => (None, files),
    };
    if files.is_empty() {
        return Err(USAGE.into());
    }
    let mut errors = 0;
    for file in files {
        let text = std::fs::read_to_string(file).map_err(|e| format!("{file}: {e}"))?;
        for d in yog_script_build::check_source(file, &text, game.as_ref()) {
            errors += usize::from(d.is_error());
            eprintln!("{d}");
        }
    }
    match errors {
        0 => Ok(()),
        n => Err(format!("{n} error(s)")),
    }
}
