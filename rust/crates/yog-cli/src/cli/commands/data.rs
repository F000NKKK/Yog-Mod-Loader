//! `yog data`: a Yog format's file between its two forms — binary on disk
//! (what the editor saves), text for people.
//!
//! - `yog data text <file>`: the file as text, on stdout (what `git diff`
//!   shows through `diff.ydata.textconv`).
//! - `yog data bin <file>...`: files still in text written binary, their
//!   comments kept as notes.
//! - `yog data git`: the mod's repository shows them as text in diffs —
//!   `.gitattributes` names the formats' files `diff=ydata`, and the
//!   repository's config makes `yog data text` that diff's textconv.

use std::path::Path;

const TEXT: &str = "text";
const BIN: &str = "bin";
const GIT: &str = "git";
/// This command: `yog data`.
pub(super) const WORDS: &[&str] = &["data"];
const USAGE: &str = "usage: yog data text <file> | yog data bin <file>... | yog data git";
/// What `.gitattributes` calls a Yog format's diff.
const DIFF_DRIVER: &str = "ydata";
const GITATTRIBUTES: &str = ".gitattributes";

pub(super) fn run(rest: &[String]) -> Result<(), String> {
    match rest.split_first() {
        Some((what, [file])) if what == TEXT => {
            print!("{}", source_text(Path::new(file))?);
            Ok(())
        }
        Some((what, files)) if what == BIN && !files.is_empty() => {
            files.iter().try_for_each(|f| to_binary(Path::new(f)))
        }
        Some((what, [])) if what == GIT => git(&yog_cli::manifest::project_root()?),
        _ => Err(USAGE.into()),
    }
}

fn source_text(file: &Path) -> Result<String, String> {
    yog_cli::files::read_source(file).map_err(|e| format!("{}: {e}", file.display()))
}

/// `file` written binary, if it's still text.
fn to_binary(file: &Path) -> Result<(), String> {
    let bytes = std::fs::read(file).map_err(|e| format!("{}: {e}", file.display()))?;
    if yog_data::source::is_binary(&bytes) {
        return Ok(());
    }
    let text = String::from_utf8(bytes).map_err(|e| format!("{}: {e}", file.display()))?;
    yog_cli::files::write_source(file, &text)?;
    eprintln!("==> {} is binary", file.display());
    Ok(())
}

/// The mod at `root`'s repository shows its Yog formats' files as text in diffs.
fn git(root: &Path) -> Result<(), String> {
    let file = root.join(GITATTRIBUTES);
    let had = std::fs::read_to_string(&file).unwrap_or_default();
    let mut text = had.clone();
    for format in yog_ids::Format::ALL.iter().filter(|f| f.is_data()) {
        let line = format!("*.{} diff={DIFF_DRIVER}", format.ext());
        if !had.lines().any(|l| l.trim() == line) {
            if !text.is_empty() && !text.ends_with('\n') {
                text.push('\n');
            }
            text.push_str(&line);
            text.push('\n');
        }
    }
    if text != had {
        std::fs::write(&file, text).map_err(|e| format!("{}: {e}", file.display()))?;
    }
    let textconv = format!("{} {} {TEXT}", yog_ids::program::CLI, WORDS[0]);
    let status = std::process::Command::new(GIT)
        .current_dir(root)
        .args(["config", &format!("diff.{DIFF_DRIVER}.textconv"), &textconv])
        .status()
        .map_err(|e| format!("git: {e}"))?;
    if !status.success() {
        return Err(format!(
            "git config failed ({status}) — is {} a repository?",
            root.display()
        ));
    }
    eprintln!("==> git shows Yog formats' files as text (`{textconv}`)");
    Ok(())
}
