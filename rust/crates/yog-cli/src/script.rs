//! A mod's Yog Script (`src/**/*.ys`, `*.yns`): each source checked by the
//! compiler (`yog-script-build`), what's wrong reported at its file, line
//! and column; errors stop the build.

pub mod jvm;

use std::path::Path;

use yog_ids::Format;

use crate::diagnostics;

/// Where a mod's code is: its scripts beside its Rust.
use crate::project::manifest::SOURCES_DIR as SOURCES;

/// What every source of a mod's Yog Script is checked with, read once:
/// what its `yog.toml` says it does (sending to the server), its files (for
/// `include_str!`), and every `.ys` source's declarations (the generated
/// ones too) — the types each source reaches of the others.
pub(crate) struct ScriptProject {
    to_server: bool,
    files: yog_script_build::FilesBeside,
    decls: Vec<(String, Vec<yog_script_analyze::ast::Decl>)>,
}

impl ScriptProject {
    /// The project of the mod at `root`.
    pub(crate) fn read(root: &Path) -> Self {
        let to_server =
            crate::project::manifest::YogToml::read(&root.join(crate::project::manifest::MANIFEST))
                .is_ok_and(|m| m.to_server);
        let files = yog_script_build::FilesBeside {
            base: root.to_path_buf(),
        };
        let env = yog_script_analyze::Env {
            files: Some(&files),
            to_server,
            ..Default::default()
        };
        let decls = jvm::sources(root)
            .unwrap_or_default()
            .into_iter()
            .map(|(name, text)| {
                let d = yog_script_analyze::declarations(&name, &text, env);
                (name, d)
            })
            .collect();
        ScriptProject {
            to_server,
            files,
            decls,
        }
    }

    /// Source `name`'s `text` checked in the project, its content files
    /// (`content`: each (kind, id)) known when they are.
    pub(crate) fn check(
        &self,
        name: &str,
        text: &str,
        content: Option<&[(String, String)]>,
    ) -> Vec<yog_data::Diagnostic> {
        let others: Vec<&[yog_script_analyze::ast::Decl]> = self
            .decls
            .iter()
            .filter(|(other, _)| other != name)
            .map(|(_, d)| d.as_slice())
            .collect();
        let project = yog_script_build::Project {
            to_server: self.to_server,
            files: Some(&self.files),
            content,
            others: &others,
            ..Default::default()
        };
        yog_script_build::check_source_using(name, text, yog_script_build::index::load, project)
    }
}

/// A script's text checked in the mod at `root` (an editor's: one file,
/// the project read for it).
pub(crate) fn check_text(
    name: &str,
    text: &str,
    root: &Path,
    content: Option<&[(String, String)]>,
) -> Vec<yog_data::Diagnostic> {
    ScriptProject::read(root).check(name, text, content)
}

/// The mod a file is in: the nearest folder up from it with a `yog.toml`.
pub(crate) fn mod_root(file: &Path) -> Option<&Path> {
    file.ancestors()
        .find(|dir| dir.join(crate::project::manifest::MANIFEST).is_file())
}

/// Checks every script of the mod at `root` (a `.yns` on each game version
/// it targets), its content files' (kind, id) `content`; an error when one
/// has errors.
pub fn check(root: &Path, content: &[(String, String)]) -> Result<(), String> {
    let dir = root.join(SOURCES);
    let files: Vec<_> = [Format::Script, Format::Nanites]
        .into_iter()
        .flat_map(|format| crate::files::of(&dir, format))
        .collect();
    if files.is_empty() {
        return Ok(());
    }
    // Every source's declarations once, not once a source.
    let project = ScriptProject::read(root);
    let mut errors = 0;
    for file in files {
        let text =
            std::fs::read_to_string(&file).map_err(|e| format!("{}: {e}", file.display()))?;
        let name = crate::shown_path(root, &file);
        let problems = project.check(&name, &text, Some(content));
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
        std::fs::write(src.join("Ok.ys"), "i32 Area(i32 w, i32 h) => w * h;\n").unwrap();
        assert_eq!(check(&root, &[]), Ok(()));
        // One source reaches another's types.
        std::fs::write(src.join("Size.ys"), "public record Size(i32 W, i32 H);\n").unwrap();
        std::fs::write(src.join("Use.ys"), "i32 W(Size s) => s.W;\n").unwrap();
        assert_eq!(check(&root, &[]), Ok(()));
        std::fs::write(src.join("Bad.ys"), "void F() {\n    break;\n}\n").unwrap();
        assert!(check(&root, &[]).is_err());
        std::fs::remove_dir_all(&root).unwrap();
    }
}
