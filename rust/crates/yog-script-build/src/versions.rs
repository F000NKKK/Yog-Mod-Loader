//! A `.yns` on the game versions it targets: `targets mc …;` matched with
//! the versions Yog runs on, the source checked against each one's
//! classes, and the problems merged — one found on some versions only says
//! which.

use std::collections::BTreeMap;

use yog_data::Diagnostic;
use yog_ids::{game_version, script_problem};
use yog_script_analyze::Env;
use yog_script_analyze::versions::targeted;

use crate::index::ClassIndex;

/// A `.yns` checked on each version it targets.
pub(crate) fn check(
    name: &str,
    text: &str,
    load: impl Fn(&str) -> Result<ClassIndex, String>,
    env: Env<'_>,
) -> Vec<Diagnostic> {
    let read = yog_script_analyze::check_in(name, text, env);
    let Some(cond) = yog_script_analyze::targets(&read.file) else {
        return read.problems;
    };
    let versions = targeted(cond);
    if versions.is_empty() || read.problems.iter().any(Diagnostic::is_error) {
        let mut problems = read.problems;
        if versions.is_empty() {
            problems.push(
                Diagnostic::error(
                    yog_data::Atom::new(script_problem::NO_VERSION),
                    format!(
                        "`targets` takes none of the versions Yog runs on ({})",
                        game_version::ALL_VALUES.join(", ")
                    ),
                )
                .in_file(name),
            );
        }
        return problems;
    }
    let mut found: BTreeMap<String, (Diagnostic, Vec<&str>)> = BTreeMap::new();
    let mut checked = 0;
    let mut out = Vec::new();
    for v in &versions {
        let game = match load(v) {
            Ok(game) => game,
            Err(e) => {
                out.push(
                    Diagnostic::warning(
                        yog_data::Atom::new(script_problem::NO_VERSION),
                        format!(
                            "the game's classes of {v} aren't at hand ({e}): not checked on it"
                        ),
                    )
                    .in_file(name),
                );
                continue;
            }
        };
        checked += 1;
        for d in yog_script_analyze::check_in(
            name,
            text,
            Env {
                game: Some(&game),
                ..env
            },
        )
        .problems
        {
            found
                .entry(d.to_string())
                .or_insert_with(|| (d, Vec::new()))
                .1
                .push(v);
        }
    }
    out.extend(found.into_values().map(|(mut d, on)| {
        if on.len() < checked {
            d.message = format!("{} (on {})", d.message, on.join(", "));
        }
        d
    }));
    out
}

#[cfg(test)]
mod tests;
