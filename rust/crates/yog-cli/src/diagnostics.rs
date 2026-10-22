//! What a build or a check finds, each as a record: what it is (a code),
//! how bad, what's wrong and where. Written for a tool — a Yog Data frame
//! each to the file `YOG_DIAGNOSTICS` names (`yog_ids::env::DIAGNOSTICS`) —
//! while the text goes on as ever (each one's `Display`).

use std::fmt;
use std::path::Path;

pub use yog_data::{Diagnostic, Severity};
use yog_sync::Mutex;
use yog_sync::atomic::{AtomicUsize, Ordering};

/// How many errors this run reported.
static ERRORS: AtomicUsize = AtomicUsize::new(0);

/// What this run reported: each one once, though the build meets it again
/// for every platform.
static REPORTED: Mutex<Vec<Diagnostic>> = Mutex::new(Vec::new());

/// Reports `d` to the tool listening (if one is), once; its text is the
/// caller's to print.
pub fn report(d: &Diagnostic) {
    {
        let mut seen = REPORTED.lock();
        if seen.contains(d) {
            return;
        }
        seen.push(d.clone());
    }
    if d.is_error() {
        ERRORS.fetch_add(1, Ordering::Relaxed);
    }
    let Some(path) = std::env::var_os(yog_ids::env::DIAGNOSTICS) else {
        return;
    };
    if let Err(e) = append(Path::new(&path), d) {
        // The text still says it; only the tool misses it.
        yog_logging::warn!(
            "couldn't report a diagnostic to {}: {e}",
            Path::new(&path).display()
        );
    }
}

/// `message` reported as an error of kind `code`, and returned to be said.
pub fn failed(code: &str, message: impl fmt::Display) -> String {
    let d = Diagnostic::error(code, message);
    report(&d);
    d.message
}

/// `d` reported, and returned to be said.
pub fn reported(d: Diagnostic) -> String {
    report(&d);
    d.to_string()
}

/// Reports every one of `ds`.
pub fn report_all<'d>(ds: impl IntoIterator<Item = &'d Diagnostic>) {
    for d in ds {
        report(d);
    }
}

/// How many errors this run has reported so far.
pub fn errors_reported() -> usize {
    ERRORS.load(Ordering::Relaxed)
}

fn append(path: &Path, d: &Diagnostic) -> std::io::Result<()> {
    let mut file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)?;
    yog_data::io::write_frame(&mut file, d)
}

/// Runs `command` (a `yog` one) with its diagnostics written to a file of
/// its own: how it went, and what it reported.
pub fn collected(
    command: &mut std::process::Command,
) -> std::io::Result<(std::process::Output, Vec<Diagnostic>)> {
    static RUNS: AtomicUsize = AtomicUsize::new(0);
    let run = RUNS.fetch_add(1, Ordering::Relaxed);
    let file = std::env::temp_dir().join(format!("{FEED_PREFIX}{}-{run}", std::process::id()));
    let output = command.env(yog_ids::env::DIAGNOSTICS, &file).output();
    let found = read_all(&file);
    // Gone with the run; never written when nothing was found.
    crate::files::remove(&file);
    Ok((output?, found))
}

/// What a run's diagnostics files are named from.
const FEED_PREFIX: &str = "yog-diagnostics-";

/// Every record in the file at `path` (none when there's no file).
pub fn read_all(path: &Path) -> Vec<Diagnostic> {
    let Ok(bytes) = std::fs::read(path) else {
        return Vec::new();
    };
    let mut rest = bytes.as_slice();
    let mut found = Vec::new();
    while let Ok(Some(d)) = yog_data::io::read_frame::<Diagnostic>(&mut rest) {
        found.push(d);
    }
    found
}

/// `ds` as the text says them, one a line, indented under a heading.
pub fn listed(ds: &[Diagnostic]) -> String {
    let lines: Vec<String> = ds.iter().map(ToString::to_string).collect();
    lines.join(LIST_SEP)
}

/// What separates the lines of a list of them (under its heading).
pub const LIST_SEP: &str = "\n  ";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_failure_is_reported_once_and_said() {
        let said = failed(yog_ids::build_problem::CONTENT, "no model");
        assert_eq!(said, "no model");
        let before = errors_reported();
        failed(yog_ids::build_problem::CONTENT, "no model");
        assert_eq!(errors_reported(), before);
    }
}
