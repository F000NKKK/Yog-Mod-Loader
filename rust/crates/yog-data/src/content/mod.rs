//! The kinds of content every Yog format knows, named in `yog-ids`, and
//! the ids of the common ones.

use substrate_data::Id;

/// The kinds every Yog format knows.
pub mod kinds {
    use yog_ids::{kind, typed};

    substrate_data::kind! {
        Item = kind::ITEM,
        Block = kind::BLOCK,
        Entity = kind::ENTITY,
        Recipe = kind::RECIPE,
        Entry = kind::ENTRY,
        Category = kind::CATEGORY,
        Fluid = kind::FLUID,
        Biome = kind::BIOME,
        Dimension = kind::DIMENSION,
        Texture = kind::TEXTURE,
        Video = kind::VIDEO,
        Material = kind::MATERIAL,
        Advancement = kind::ADVANCEMENT,
        Tag = typed::TAG,
        Res = typed::RES,
        Lang = typed::LANG,
        Image = typed::IMAGE,
    }
}

/// An item: `@item:minecraft:stick`.
pub type ItemId = Id<kinds::Item>;
/// A block: `@block:minecraft:furnace[facing=north]`.
pub type BlockId = Id<kinds::Block>;
/// An entity: `@entity:minecraft:pig`.
pub type EntityId = Id<kinds::Entity>;
/// A recipe: `@recipe:minecraft:furnace`.
pub type RecipeId = Id<kinds::Recipe>;
