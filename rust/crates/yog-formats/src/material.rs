//! `.ymaterial` — a material: how a surface is drawn — the textures its
//! model's faces name (`#side`, `#top`, …) and its render layer. Its id is
//! its path under `resources/` without the extension, named elsewhere as
//! `@material:<mod id>:block/ruby`; blocks, items and entities take one
//! instead of listing textures themselves.

use yog_data::{Ordered, Schema};

use crate::content::Texture;

/// A `.ymaterial`.
#[derive(Schema, Debug, Clone, PartialEq, Default)]
pub struct MaterialFile {
    /// The textures, by the slot a model's faces name (`all`, `side`, `top`, `particle`…).
    pub textures: Ordered<Texture>,
    /// How it's blended: `solid` (the default), `cutout`, `cutout_mipped`, `translucent`.
    pub render_layer: Option<String>,
}
