//! `yog publish exports [--dry-run]`: the mod's exports crate published.

use crate::cli::flags::{DRY_RUN, HELP};

pub(super) const WORDS: &[&str] = &["publish"];
const USAGE: &str = "usage: yog publish exports [--dry-run]";
const EXPORTS: &str = "exports";

pub(super) fn run(rest: &[String]) -> Result<(), String> {
    // Publishing can't be undone: anything but `--dry-run` (a typo, `--help`) stops here.
    let flags: Vec<&str> = rest.iter().skip(1).map(String::as_str).collect();
    let other = flags.iter().find(|a| **a != DRY_RUN);
    match (rest.first().map(String::as_str), other) {
        (Some(EXPORTS), Some(o)) if HELP.contains(o) => {
            println!(
                "{USAGE}\n\nPublishes this mod's exports crate (`#[yog_export]` items) to crates.io; --dry-run only generates and packages it."
            );
            Ok(())
        }
        (Some(EXPORTS), Some(o)) => Err(format!("unknown flag {o} — {USAGE}")),
        (Some(EXPORTS), None) => crate::cli::project::publish::publish_exports(!flags.is_empty()),
        (Some(h), _) if HELP.contains(&h) => {
            println!("{USAGE}");
            Ok(())
        }
        (Some(other), _) => Err(format!("unknown publish subcommand: {other}")),
        (None, _) => Err(USAGE.into()),
    }
}
