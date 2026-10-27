//! A value that isn't what a format's own form takes.

use substrate_data::{Problem, Reader, ValueRef};

/// "expected `what`, found …": for a form [`substrate_data::Expected`]
/// has no word for (`"auto"`, `"full"` or six numbers).
pub fn expected(cx: &mut Reader<'_>, what: &str, found: &ValueRef<'_>) {
    cx.problem_of(Problem::Expected, format!("expected {what}, found {}", found.kind()));
}
