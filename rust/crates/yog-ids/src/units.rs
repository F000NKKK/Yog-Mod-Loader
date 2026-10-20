//! The game's units: what every part of Yog measures blocks and stacks in.

/// A block's pixels along an edge: models are drawn in them (0–16 a block).
pub const BLOCK_PIXELS: u32 = 16;
/// The most of an item a stack holds.
pub const MAX_STACK: u32 = 64;
