//! `yog build [debug|release] [--dev]`: the mod built and packaged.
//! `debug`: the `--release` build with its debug info kept (for
//! `yog-symbols`/`yog-debugger`); `release`: optimized, the debug info split
//! out beside it; `--dev`: for a game that hot-reloads it (Yog-IDLE,
//! dev.sh) — the `.yog` carries what hot reload checks.

use crate::cli::flags::{DEV, has};
use crate::cli::pipeline::{self, Platforms, symbols::Symbols};

pub(super) const WORDS: &[&str] = &["build"];

pub(super) fn run(rest: &[String]) -> Result<(), String> {
    let symbols = Symbols::of(rest.first().map(String::as_str));
    pipeline::build(symbols, has(rest, DEV), Platforms::Selected).map(|_| ())
}
