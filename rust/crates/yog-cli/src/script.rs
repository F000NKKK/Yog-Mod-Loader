//! A mod's Yog Script (`src/**/*.ys`, `*.yns`): each source checked by the
//! compiler (`yog-script-build`), what's wrong reported at its file, line
//! and column; errors stop the build.

use std::path::Path;

use yog_ids::Format;

use crate::diagnostics;

/// Where a mod's code is: its scripts beside its Rust.
const SOURCES: &str = "src";

/// Checks every script of the mod at `root` against the game's classes of
/// `mc` (the version its vanilla content is of); an error when one has errors.
pub fn check(root: &Path, mc: Option<&str>) -> Result<(), String> {
    let dir = root.join(SOURCES);
    let files: Vec<_> = [Format::Script, Format::Nanites]
        .into_iter()
        .flat_map(|format| crate::files::of(&dir, format))
        .collect();
    if files.is_empty() {
        return Ok(());
    }
    let game = mc.and_then(game_index);
    let mut errors = 0;
    for file in files {
        let text =
            std::fs::read_to_string(&file).map_err(|e| format!("{}: {e}", file.display()))?;
        let name = crate::shown_path(root, &file);
        let problems = yog_script_build::check_source(&name, &text, game.as_ref());
        errors += problems.iter().filter(|d| d.is_error()).count();
        diagnostics::report_all(&problems);
        for d in &problems {
            match d.is_error() {
                true => yog_logging::error!("{d}"),
                false => yog_logging::warn!("{d}"),
            }
        }
    }
    match errors {
        0 => Ok(()),
        n => Err(format!("{n} error(s) in the mod's Yog Script")),
    }
}

/// The game's classes of `mc`, from the cache or built; without them (no
/// network the first time) the scripts are checked by what they declare.
fn game_index(mc: &str) -> Option<yog_script_build::index::ClassIndex> {
    yog_script_build::index::load(mc)
        .map_err(|e| {
            yog_logging::warn!(
                "the game's classes of {mc} aren't at hand ({e}): scripts checked without them"
            )
        })
        .ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_mods_scripts_are_checked() {
        let root = std::env::temp_dir().join(format!("yog-script-check-{}", std::process::id()));
        let src = root.join(SOURCES).join("power");
        std::fs::create_dir_all(&src).unwrap();
        std::fs::write(src.join("ok.ys"), "int Area(int w, int h) => w * h;\n").unwrap();
        assert_eq!(check(&root, None), Ok(()));
        std::fs::write(src.join("bad.ys"), "void F() {\n    break;\n}\n").unwrap();
        assert!(check(&root, None).is_err());
        std::fs::remove_dir_all(&root).unwrap();
    }
}
