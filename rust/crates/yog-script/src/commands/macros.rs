//! `--macros <file.ys>` before a command's files: the procedural macros
//! the files are checked with.

use yog_script_build::MacroLibrary;

/// The flag naming a macros' source.
const FLAG: &str = "--macros";

/// `rest` split into the macros' library (when any is named) and the files.
pub(super) fn split(rest: &[String]) -> Result<(Option<MacroLibrary>, Vec<String>), String> {
    let mut sources = Vec::new();
    let mut files = Vec::new();
    let mut args = rest.iter();
    while let Some(a) = args.next() {
        if a == FLAG {
            let path = args.next().ok_or_else(|| format!("{FLAG} takes a file"))?;
            let text = std::fs::read_to_string(path).map_err(|e| format!("{path}: {e}"))?;
            sources.push((path.clone(), text));
        } else {
            files.push(a.clone());
        }
    }
    if sources.is_empty() {
        return Ok((None, files));
    }
    let lib = MacroLibrary::load(&sources).map_err(|problems| {
        problems.iter().for_each(|d| eprintln!("{d}"));
        format!("the macros ({}) have errors", sources.len())
    })?;
    Ok((Some(lib), files))
}
