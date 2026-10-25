//! A mod's Yog Script (`src/**/*.ys`, `*.yns`): each source checked by the
//! compiler (`yog-script-build`), what's wrong reported at its file, line
//! and column; errors stop the build.

pub mod jvm;

use std::path::Path;

use yog_ids::Format;

use crate::diagnostics;

/// Where a mod's code is: its scripts beside its Rust.
use crate::project::manifest::SOURCES_DIR as SOURCES;

/// A script's text checked in the mod at `root`: what its `yog.toml` says
/// it does (sending to the server) known.
pub(crate) fn check_text(name: &str, text: &str, root: &Path) -> Vec<yog_data::Diagnostic> {
    let to_server =
        crate::project::manifest::YogToml::read(&root.join(crate::project::manifest::MANIFEST))
            .is_ok_and(|m| m.to_server);
    let project = yog_script_build::Project {
        to_server,
        ..Default::default()
    };
    yog_script_build::check_source_using(name, text, yog_script_build::index::load, project)
}

/// The mod a file is in: the nearest folder up from it with a `yog.toml`.
pub(crate) fn mod_root(file: &Path) -> Option<&Path> {
    file.ancestors()
        .find(|dir| dir.join(crate::project::manifest::MANIFEST).is_file())
}

/// Checks every script of the mod at `root` (a `.yns` on each game version
/// it targets); an error when one has errors.
pub fn check(root: &Path) -> Result<(), String> {
    let dir = root.join(SOURCES);
    let files: Vec<_> = [Format::Script, Format::Nanites]
        .into_iter()
        .flat_map(|format| crate::files::of(&dir, format))
        .collect();
    if files.is_empty() {
        return Ok(());
    }
    let mut errors = 0;
    for file in files {
        let text =
            std::fs::read_to_string(&file).map_err(|e| format!("{}: {e}", file.display()))?;
        let name = crate::shown_path(root, &file);
        let problems = check_text(&name, &text, root);
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_mods_scripts_are_checked() {
        let root = std::env::temp_dir().join(format!("yog-script-check-{}", std::process::id()));
        let src = root.join(SOURCES).join("power");
        std::fs::create_dir_all(&src).unwrap();
        std::fs::write(src.join("ok.ys"), "int Area(int w, int h) => w * h;\n").unwrap();
        assert_eq!(check(&root), Ok(()));
        std::fs::write(src.join("bad.ys"), "void F() {\n    break;\n}\n").unwrap();
        assert!(check(&root).is_err());
        std::fs::remove_dir_all(&root).unwrap();
    }
}
