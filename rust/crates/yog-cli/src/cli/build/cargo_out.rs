//! Cargo run for a mod, its compiler's messages read as records: each
//! said as the compiler renders it and reported (`yog_cli::diagnostics`)
//! with its code and place; cargo failing on its own — resolving,
//! fetching, its toolchain — reported with what it said.

use std::io::{BufRead, BufReader};
use std::path::{Component, Path, PathBuf};
use std::process::{ChildStderr, Command, ExitStatus, Stdio};

use serde_json::Value;
use yog_cli::diagnostics::{self, Diagnostic, Severity};
use yog_ids::build_problem;

/// The flag that makes cargo write its messages as JSON lines on stdout.
const JSON_MESSAGES: &str = "--message-format=json";
/// A JSON line's kind, and the kind that's the compiler's message.
const REASON: &str = "reason";
const COMPILER_MESSAGE: &str = "compiler-message";
/// The tool the compiler's messages come from.
const RUSTC: &str = "rustc";
/// The tool cargo's own failures come from.
const CARGO: &str = "cargo";
/// Where cargo's own failure starts in what it says.
const ERROR_LINE: &str = "error";
/// The compiler's closing summaries, not problems of their own.
const ABORTING: &str = "aborting due to";
/// Cargo's count of a crate's warnings (`… (lib) generated 2 warnings`),
/// each already said with its own text.
const GENERATED: &str = ") generated ";
const WARNING_LINE: &str = "warning:";

/// Runs `cmd` (a cargo command in `build_dir`, the mod's generated crate
/// under `root`): its messages said and reported as they come; whether it
/// succeeded.
pub(crate) fn run(cmd: &mut Command, build_dir: &Path, root: &Path) -> std::io::Result<bool> {
    let mut child = cmd
        .arg(JSON_MESSAGES)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;
    let stderr = child.stderr.take().map(|e| std::thread::spawn(|| said(e)));
    let mut compiler_failed = false;
    if let Some(stdout) = child.stdout.take() {
        for line in BufReader::new(stdout).lines() {
            if let Ok(v) = serde_json::from_str::<Value>(&line?) {
                compiler_failed |= message(&v, build_dir, root) == Some(Severity::Error);
            }
        }
    }
    let status = child.wait()?;
    let stderr = stderr.and_then(|t| t.join().ok()).unwrap_or_default();
    if !status.success() && !compiler_failed {
        cargo_failed(status, &stderr);
    }
    Ok(status.success())
}

/// Cargo's own lines (its progress, its failures) passed on as they come;
/// all it said.
fn said(stderr: ChildStderr) -> String {
    let mut all = String::new();
    for line in BufReader::new(stderr).lines().map_while(Result::ok) {
        if is_warning_count(&line) {
            continue;
        }
        eprintln!("{line}");
        all.push_str(&line);
        all.push('\n');
    }
    all
}

/// Cargo's count of a crate's warnings.
fn is_warning_count(line: &str) -> bool {
    line.starts_with(WARNING_LINE) && line.contains(GENERATED)
}

/// Warnings that come from the zig cross-linking toolchain itself, not from
/// the mod's code, and that the mod author can't act on:
/// - "xcrun … MacOSX.sdk failed" — rustc probing for a macOS SDK when
///   cross-linking Apple targets (zig handles the actual linking).
/// - "linker stderr: ignoring deprecated linker optimization setting" —
///   rustc always passes `-Wl,-O1` to gnu-flavored linkers; zig's ld (0.16+)
///   ignores it and says so on every link.
fn is_benign_warning(t: &str) -> bool {
    t.starts_with(WARNING_LINE)
        && ((t.contains("xcrun") && t.contains("failed"))
            || (t.contains("linker stderr")
                && t.contains("deprecated linker optimization setting")))
}

/// One JSON line: a compiler message said and reported; how bad it was.
fn message(v: &Value, build_dir: &Path, root: &Path) -> Option<Severity> {
    if v.get(REASON).and_then(Value::as_str) != Some(COMPILER_MESSAGE) {
        return None;
    }
    let m = v.get("message")?;
    let rendered = m
        .get("rendered")
        .and_then(Value::as_str)
        .unwrap_or_default();
    let text = m.get("message").and_then(Value::as_str).unwrap_or_default();
    let severity = match m.get("level").and_then(Value::as_str) {
        Some("error") => Severity::Error,
        Some("warning") => Severity::Warning,
        _ => Severity::Note,
    };
    if is_benign_warning(rendered.trim_start()) || text.starts_with(ABORTING) {
        return None;
    }
    eprint!("{rendered}");
    if severity == Severity::Note {
        return Some(severity);
    }
    let code = m
        .pointer("/code/code")
        .and_then(Value::as_str)
        .unwrap_or(build_problem::COMPILER);
    let mut d = Diagnostic::of(severity, code, text).by(RUSTC);
    if let Some(span) = primary_span(m) {
        place(&mut d, span, build_dir, root);
    }
    diagnostics::report(&d);
    Some(severity)
}

/// The span the message is about.
fn primary_span(m: &Value) -> Option<&Value> {
    m.get("spans")?
        .as_array()?
        .iter()
        .find(|s| s.get("is_primary").and_then(Value::as_bool) == Some(true))
}

/// `d` at `span`: its file relative to the mod's root when it's the mod's.
fn place(d: &mut Diagnostic, span: &Value, build_dir: &Path, root: &Path) {
    let num = |k: &str| {
        span.get(k)
            .and_then(Value::as_u64)
            .and_then(|n| u32::try_from(n).ok())
    };
    if let Some(file) = span.get("file_name").and_then(Value::as_str) {
        let full = normalized(&build_dir.join(file));
        let shown = full
            .strip_prefix(root)
            .map_or(full.clone(), Path::to_path_buf);
        *d = std::mem::take(d).in_file(yog_cli::slash_path(&shown));
    }
    d.line = num("line_start");
    d.col = num("column_start");
    d.end_line = num("line_end");
    d.end_col = num("column_end");
}

/// `p` with its `..` and `.` resolved (the crate names the mod's sources `../src/…`).
fn normalized(p: &Path) -> PathBuf {
    let mut out = PathBuf::new();
    for c in p.components() {
        match c {
            Component::ParentDir => {
                out.pop();
            }
            Component::CurDir => {}
            other => out.push(other),
        }
    }
    out
}

/// Cargo failed without the compiler saying why: what cargo said, from its error on.
fn cargo_failed(status: ExitStatus, stderr: &str) {
    let said: Vec<&str> = stderr
        .lines()
        .skip_while(|l| !l.trim_start().starts_with(ERROR_LINE))
        .collect();
    let message = if said.is_empty() {
        format!("cargo failed ({status})")
    } else {
        said.join("\n")
    };
    diagnostics::report(&Diagnostic::error(build_problem::CARGO, message).by(CARGO));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_mod_source_is_named_from_the_mods_root() {
        let root = Path::new("/m");
        let span = serde_json::json!({ "file_name": "../src/lib.rs", "line_start": 3, "column_start": 5, "line_end": 3, "column_end": 9 });
        let mut d = Diagnostic::error("E0308", "mismatched types");
        place(&mut d, &span, &root.join(".yog-build"), root);
        assert_eq!(d.file.as_deref(), Some("src/lib.rs"));
        assert_eq!((d.line, d.col, d.end_col), (Some(3), Some(5), Some(9)));
    }

    #[test]
    fn the_zig_linker_note_is_benign_and_counts_are_dropped() {
        assert!(is_benign_warning(
            "warning: linker stderr: ignoring deprecated linker optimization setting '1'"
        ));
        assert!(!is_benign_warning("warning: unused variable: `x`"));
        assert!(is_warning_count(
            "warning: `hexcasting` (lib) generated 1 warning"
        ));
        assert!(!is_warning_count(
            "    Finished `release` profile [optimized] target(s)"
        ));
    }
}
