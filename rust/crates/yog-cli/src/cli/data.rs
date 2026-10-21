//! `yog data`: a Yog format's file between its two forms — binary on disk
//! (what the editor saves), text for people.
//!
//! - `yog data text <file>`: the file as text, on stdout (what `git diff`
//!   shows through `diff.ydata.textconv`).
//! - `yog data bin <file>...`: files still in text written binary, their
//!   comments kept as notes.

use std::path::Path;

const TEXT: &str = "text";
const BIN: &str = "bin";
const USAGE: &str = "usage: yog data text <file> | yog data bin <file>...";

pub(super) fn data(rest: &[String]) -> Result<(), String> {
    match rest.split_first() {
        Some((what, [file])) if what == TEXT => {
            print!("{}", source_text(Path::new(file))?);
            Ok(())
        }
        Some((what, files)) if what == BIN && !files.is_empty() => {
            files.iter().try_for_each(|f| to_binary(Path::new(f)))
        }
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
