//! The game's units a number may be written in (`20t`, `1.5s`, `3b`):
//! their types' names and the suffixes that say them.

pub const TICKS: &str = "Ticks";
pub const SECONDS: &str = "Seconds";
pub const BLOCKS: &str = "Blocks";
/// Every unit the game has (a mod may declare its own).
pub const GAME_UNITS: [&str; 3] = [TICKS, SECONDS, BLOCKS];

/// `20t`: ticks.
pub const TICK_SUFFIX: &str = "t";
/// `1s`, `250ms`, `2m`, `1h`: seconds, and its multiples — each suffix
/// with how many seconds one of it is.
pub const SECOND_SUFFIXES: [(&str, f64); 4] =
    [("s", 1.0), ("ms", 0.001), ("m", 60.0), ("h", 3600.0)];
/// `3b`: blocks.
pub const BLOCK_SUFFIX: &str = "b";
