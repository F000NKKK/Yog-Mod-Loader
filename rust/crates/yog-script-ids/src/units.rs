//! The game's units a number may be written in (`20t`, `1.5s`, `3b`):
//! their types' names and the suffixes that say them.

pub const TICKS: &str = "Ticks";
pub const SECONDS: &str = "Seconds";
pub const BLOCKS: &str = "Blocks";

/// `20t`: ticks.
pub const TICK_SUFFIX: &str = "t";
/// `1s`, `250ms`, `2m`, `1h`: seconds, and its multiples.
pub const SECOND_SUFFIXES: [&str; 4] = ["s", "ms", "m", "h"];
/// `3b`: blocks.
pub const BLOCK_SUFFIX: &str = "b";
