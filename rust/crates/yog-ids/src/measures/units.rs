//! The game's units: what every part of Yog measures blocks and stacks in.

/// Each unit a constant, and every one listed (`ALL`: the Java hosts' `YogNames.Units`).
macro_rules! units {
    ($($(#[$doc:meta])* $name:ident = $value:expr;)*) => {
        $($(#[$doc])* pub const $name: u32 = $value;)*
        /// Every unit, by name.
        pub const ALL: &[(&str, u32)] = &[$((stringify!($name), $name)),*];
    };
}

units! {
    /// A block's pixels along an edge: models are drawn in them (0–16 a block).
    BLOCK_PIXELS = 16;
    /// An item's icon's side, in GUI pixels.
    ITEM_PIXELS = 16;
    /// The most of an item a stack holds.
    MAX_STACK = 64;
    /// A chunk's side is `1 << CHUNK_BITS` blocks: a block's chunk is its
    /// coordinate shifted right by it, its place in it the low bits.
    CHUNK_BITS = 4;
}

/// A block's place along a chunk's side: the coordinate's low bits.
pub const CHUNK_MASK: i32 = (1 << CHUNK_BITS) - 1;
