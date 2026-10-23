//! `yog ui gen`, `yog res check`, `yog content check`: one step of the
//! build, without building.

use std::path::{Path, PathBuf};

use yog_cli::diagnostics::{self, Diagnostic};
use yog_cli::manifest::{MANIFEST, YogToml, project_root};
use yog_cli::{content, yres, yui, yui_component};

use super::prepare::resolve_metadata;

/// `yog ui gen [files...]` — regenerates `.yog-build/generated/**/*.g.rs`
/// for the given `.yui` screen files (each in the project whose `yog.toml`
/// is above it), or for every `src/**/*.yui` of the current project.
/// `yog build`/`yog check` do the same before compiling; this is for editors
/// (Yog-IDLE runs it on save) and for checking a screen without a build.
pub(crate) fn ui_gen(files: &[String]) -> Result<(), String> {
    if files.is_empty() {
        let root = project_root()?;
        load_resource_refs(&root);
        for file in yui::find_yui_files(&root.join("src")) {
            let (out, warnings) =
                yui::generate_file(&root, &file).map_err(diagnostics::reported)?;
            eprintln!("==> {} -> {}", file.display(), out.display());
            warn_all(&warnings);
        }
        warn_index(&root)?;
        return Ok(());
    }
    for file in files.iter().map(PathBuf::from) {
        let root = yui::project_root(&file).ok_or_else(|| {
            format!(
                "{}: not inside a yog project (no yog.toml above it)",
                file.display()
            )
        })?;
        load_resource_refs(&root);
        let (out, warnings) = yui::generate_file(&root, &file).map_err(diagnostics::reported)?;
        eprintln!("==> {} -> {}", file.display(), out.display());
        warn_all(&warnings);
        warn_index(&root)?;
    }
    Ok(())
}

/// Says (and reports) each of `warnings`.
pub(crate) fn warn_all(warnings: &[Diagnostic]) {
    for w in warnings {
        eprintln!("    warning: {w}");
        diagnostics::report(w);
    }
}

/// Writes the components' index, saying what's wrong with it.
pub(crate) fn warn_index(root: &Path) -> Result<(), String> {
    if let (_, Some(w)) = yui_component::write_index(root)? {
        warn_all(&[Diagnostic::warning(yog_ids::build_problem::SCREEN, w)]);
    }
    Ok(())
}

/// Makes the project's `.yres` resources available to `.yui` generation
/// (`text="@Strings.title"`). Broken resources are reported but don't stop
/// the screen from generating — `yog build` fails on them anyway.
pub(crate) fn load_resource_refs(root: &Path) {
    let refs = YogToml::read(&root.join(MANIFEST))
        .and_then(|meta| yres::compile(root, &meta.id))
        .map(|c| c.refs)
        .unwrap_or_else(|e| {
            eprintln!("warning: {e}");
            yres::Refs::default()
        });
    yui::set_resource_refs(&refs);
}

/// `yog res check` — what `yog build` does with `resources/**/*.yres`,
/// without building: errors with file:line, untranslated texts per locale,
/// and fresh accessors in `.yog-build/generated/resources/`.
pub(crate) fn res_check() -> Result<(), String> {
    let root = project_root()?;
    let mut meta = YogToml::read(&root.join(MANIFEST))?;
    let compiled = yres::compile(&root, &meta.id)?;
    yres::write_sources(&root, &compiled)?;
    resolve_metadata(&mut meta, &compiled.refs)?;
    for w in &compiled.warnings {
        eprintln!("warning: {w}");
    }
    eprintln!(
        "==> {} resource file(s) OK{}",
        compiled.files,
        if compiled.warnings.is_empty() {
            String::new()
        } else {
            format!(", {} warning(s)", compiled.warnings.len())
        }
    );
    Ok(())
}

/// `yog content check` — what `yog build` does with `content/**/*.yblock`
/// and `*.yitem`, without building: errors with file:line, a fresh
/// `.yog-build/generated/yog-content.g.rs`.
pub(crate) fn content_check() -> Result<(), String> {
    let root = project_root()?;
    let meta = YogToml::read(&root.join(MANIFEST))?;
    let refs = yres::compile(&root, &meta.id)
        .map(|c| c.refs)
        .unwrap_or_else(|e| {
            eprintln!("warning: {e}");
            yres::Refs::default()
        });
    let compiled = content::compile(&root, &meta.id, &refs)?;
    content::write_source(&root, &compiled)?;
    for w in &compiled.warnings {
        eprintln!("warning: {w}");
    }
    eprintln!(
        "==> {} block(s), {} item(s) OK{}",
        compiled.blocks,
        compiled.items,
        if compiled.warnings.is_empty() {
            String::new()
        } else {
            format!(", {} warning(s)", compiled.warnings.len())
        }
    );
    Ok(())
}
