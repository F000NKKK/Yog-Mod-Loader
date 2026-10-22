//! `.yui` screen files (the screen designer's format) — regenerating their
//! code (`.yog-build/generated/**/*.g.rs`, what the code-behind pulls in
//! with `yog_api::ui_include!`) when one is saved, through `yog ui gen` —
//! and the same for `.yres` resource files (`resources/<Set>.g.rs`,
//! through `yog res check`).

use std::path::PathBuf;
use yog_ids::Format;

/// Runs `yog ui gen <path>` in the file's folder, so a saved screen's
/// generated `foo.g.rs` is current without a full build (`yog` finds the
/// project from the `yog.toml` above the file). The result carries what
/// `yog` found either way — its errors and warnings go to the Error List;
/// `Err` only when `yog` couldn't run at all.
pub fn codegen(path: &str) -> Result<Codegen, String> {
    let file = PathBuf::from(path);
    let args: &[&str] = match Format::of_path(&file) {
        Some(Format::Ui) => &["ui", "gen"],
        // Regenerates `.yog-build/generated/resources/<Set>.g.rs` (and checks the set).
        Some(Format::Res) => &["res", "check"],
        // Regenerates `.yog-build/generated/yog-content.g.rs` and its parts (and checks every content file).
        Some(Format::Block | Format::Item) => &["content", "check"],
        _ => return Err(format!("not a .yui, .yres, .yblock or .yitem file: {path}")),
    };
    let mut command = crate::design::this_cli();
    command.args(args);
    if args[0] == "ui" {
        command.arg(&file);
    }
    // `yog res check` / `yog content check` work on the project in the current folder.
    let project = file
        .ancestors()
        .skip(1)
        .find(|d| crate::design::project::path(d).is_file());
    let dir = (if args[0] != "ui" { project } else { None })
        .or(file.parent())
        .filter(|d| d.is_dir());
    if let Some(dir) = dir {
        command.current_dir(dir);
    }
    let (output, problems) = crate::diagnostics::collected(&mut command)
        .map_err(|e| format!("failed to run yog: {e}"))?;
    let failed = || {
        let errors: Vec<String> = problems
            .iter()
            .filter(|d| d.is_error())
            .map(ToString::to_string)
            .collect();
        if errors.is_empty() {
            format!("yog {} failed ({})", args.join(" "), output.status)
        } else {
            errors.join("\n")
        }
    };
    Ok(Codegen {
        error: (!output.status.success()).then(failed),
        root: project
            .map(|d| d.to_string_lossy().into_owned())
            .unwrap_or_default(),
        problems,
    })
}

/// One regeneration: the failure's message, and what `yog` found — each
/// record's file relative to `root`, the project's folder.
#[derive(yog_data::Schema)]
pub struct Codegen {
    pub error: Option<String>,
    pub problems: Vec<yog_data::Diagnostic>,
    pub root: String,
}
