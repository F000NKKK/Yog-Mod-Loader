//! Content registration — declare custom items and blocks from Rust.
//!
//! Definitions are collected at registration time and handed to the host, which
//! registers real `Item`/`Block` objects before the game's registries freeze.
//! (Models/textures are assets you ship separately; an unstyled item/block still
//! works — it just renders with the missing-texture look.)
//!
//! ```
//! # use yog_registry::{ItemDef, BlockDef};
//! ItemDef::new("mymod:ruby").max_stack(16);
//! BlockDef::new("mymod:ruby_block").strength(3.0, 6.0);
//! ```

/// A custom item to register, identified by `namespace:path`.
#[derive(Debug, Clone)]
pub struct ItemDef {
    pub id: String,
    pub max_stack: u8,
}

impl ItemDef {
    pub fn new(id: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            max_stack: 64,
        }
    }

    /// Maximum stack size (default 64).
    pub fn max_stack(mut self, n: u8) -> Self {
        self.max_stack = n;
        self
    }
}

/// A custom block to register; it also gets a matching item so it's obtainable.
#[derive(Debug, Clone)]
pub struct BlockDef {
    pub id: String,
    pub hardness: f32,
    pub resistance: f32,
}

impl BlockDef {
    pub fn new(id: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            hardness: 1.5,
            resistance: 6.0,
        }
    }

    /// Mining hardness and blast resistance (defaults 1.5 / 6.0).
    pub fn strength(mut self, hardness: f32, resistance: f32) -> Self {
        self.hardness = hardness;
        self.resistance = resistance;
        self
    }
}
