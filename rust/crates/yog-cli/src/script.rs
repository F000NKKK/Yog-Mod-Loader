//! A mod's Yog Script (`src/**/*.ys`, `*.yns`): each source read and its
//! names resolved, what's wrong reported at its file, line and column.

use std::path::Path;

use yog_ids::Format;

use crate::diagnostics;

/// Where a mod's code is: its scripts beside its Rust.
const SOURCES: &str = "src";

/// Checks every script of the mod at `root`; an error when one has errors.
pub fn check(root: &Path) -> Result<(), String> {
    let dir = root.join(SOURCES);
    let mut errors = 0;
    for format in [Format::Script, Format::Nanites] {
        for file in crate::files::of(&dir, format) {
            let text =
                std::fs::read_to_string(&file).map_err(|e| format!("{}: {e}", file.display()))?;
            let name = file
                .strip_prefix(root)
                .unwrap_or(&file)
                .display()
                .to_string();
            let checked = yog_script::check(&name, &text);
            errors += checked
                .problems
                .iter()
                .filter(|d| d.severity == yog_data::Severity::Error)
                .count();
            diagnostics::report_all(&checked.problems);
            for d in &checked.problems {
                eprintln!("{d}");
            }
        }
    }
    match errors {
        0 => Ok(()),
        n => Err(format!("{n} error(s) in the mod's Yog Script")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_mods_scripts_are_checked() {
        let root = std::env::temp_dir().join(format!("yog-script-check-{}", std::process::id()));
        let src = root.join(SOURCES).join("power");
        std::fs::create_dir_all(&src).unwrap();
        std::fs::write(src.join("ok.ys"), "fn area(w: Int, h: Int) = w * h\n").unwrap();
        assert_eq!(check(&root), Ok(()));
        std::fs::write(src.join("bad.ys"), "fn f() {\n    break\n}\n").unwrap();
        assert!(check(&root).is_err());
        std::fs::remove_dir_all(&root).unwrap();
    }
}
