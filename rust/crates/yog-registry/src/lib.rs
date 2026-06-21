//! Content registration — declare custom items and blocks from Rust.
//!
//! Definitions are collected at registration time and handed to the host, which
//! registers real `Item`/`Block` objects before the game's registries freeze,
//! puts them in a "Yog" creative tab, and applies their name/description.
//! (Textures, models and recipes are assets/data shipped in the `.yog` and
//! served to the game.)
//!
//! ```
//! # use yog_registry::{ItemDef, BlockDef};
//! ItemDef::new("mymod:ruby").name("Ruby").tooltip("A shiny gem.").max_stack(16);
//! BlockDef::new("mymod:ruby_block").name("Ruby Block").strength(3.0, 6.0);
//! ```

/// A custom item to register, identified by `namespace:path`.
#[derive(Debug, Clone)]
pub struct ItemDef {
    pub id: String,
    pub max_stack: u8,
    pub name: Option<String>,
    pub tooltip: Option<String>,
}

impl ItemDef {
    pub fn new(id: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            max_stack: 64,
            name: None,
            tooltip: None,
        }
    }

    /// Maximum stack size (default 64).
    pub fn max_stack(mut self, n: u8) -> Self {
        self.max_stack = n;
        self
    }

    /// Display name shown in-game (overrides the translation key).
    pub fn name(mut self, name: impl Into<String>) -> Self {
        self.name = Some(name.into());
        self
    }

    /// A tooltip line shown on hover.
    pub fn tooltip(mut self, tooltip: impl Into<String>) -> Self {
        self.tooltip = Some(tooltip.into());
        self
    }
}

/// A custom block to register; it also gets a matching item.
#[derive(Debug, Clone)]
pub struct BlockDef {
    pub id: String,
    pub hardness: f32,
    pub resistance: f32,
    pub name: Option<String>,
}

impl BlockDef {
    pub fn new(id: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            hardness: 1.5,
            resistance: 6.0,
            name: None,
        }
    }

    /// Mining hardness and blast resistance (defaults 1.5 / 6.0).
    pub fn strength(mut self, hardness: f32, resistance: f32) -> Self {
        self.hardness = hardness;
        self.resistance = resistance;
        self
    }

    /// Display name shown in-game.
    pub fn name(mut self, name: impl Into<String>) -> Self {
        self.name = Some(name.into());
        self
    }
}
