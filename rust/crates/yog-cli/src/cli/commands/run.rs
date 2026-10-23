//! `yog run <target> | <loader> <mc> [--debug] [--dev]`, `yog run --list [--json]`.

use crate::cli::flags::{DEBUG, DEV, LIST, has, words};
use crate::cli::pipeline::symbols::Symbols;

pub(super) const WORDS: &[&str] = &["run"];
const USAGE: &str =
    "usage: yog run <target> | <loader> <mc> [--debug] [--dev]; yog run --list [--json]";

pub(super) fn run(rest: &[String]) -> Result<(), String> {
    if has(rest, LIST) {
        return crate::cli::run::list_run_targets();
    }
    let symbols = if has(rest, DEBUG) {
        Symbols::Kept
    } else {
        Symbols::Stripped
    };
    let dev = has(rest, DEV);
    match words(rest).as_slice() {
        // `yog run fabric 26.2` is `yog run fabric-26.2`.
        [loader, mc] => {
            crate::cli::run::run_target(&[*loader, mc].join(yog_ids::sep::HYPHEN), symbols, dev)
        }
        [id] => crate::cli::run::run_target(id, symbols, dev),
        _ => Err(USAGE.into()),
    }
}
