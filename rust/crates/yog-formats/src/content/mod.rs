//! The `.yitem` and `.yblock` formats in Yog Data: an item or a block, its
//! model (a template or elements) and properties. `yog build` turns them
//! into the game's models, block states, loot tables and the Rust glue.

use yog_data::with::Opt;
use yog_data::{Schema, Text};

mod block;
mod item;
mod kind;
mod model;
mod refs;
pub mod vocab;

pub use block::{BlockProps, BlockShape, DropCount, DropItem, Drops, RenderLayer, TriggeredBy};
pub use item::{ArmorProps, BlockItemProps, Food, HangsProps, ItemProps, PotionProps, ToolProps};
pub use kind::{BlockKind, KIND_FIELDS, KindModel, KindPart, KindRef};
pub use model::{
    Axis, Connect, Display, Element, ElementRotation, Face, Faces, Facing, GuiLight, Model,
    ModelNames, Placement, Rotation, SLOT_MARK, States, Texture, Transform,
};
pub use refs::{
    BlockTag, LevelRef, MaterialKind, MaterialRef, OneOrMany, SoundRef, ToolKindRef, ToolTag,
};

/// Registered by `yog_api::content!()` (`auto`), or by the mod's code.
#[derive(Schema, Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Register {
    #[default]
    Auto,
    Manual,
}

/// A schema enum's name as written.
pub fn word<T: Schema>(v: &T) -> String {
    v.write().as_str().unwrap_or_default().to_string()
}

/// A `.yitem`.
#[derive(Schema, Debug, Clone)]
pub struct ItemFile {
    /// The file format; 1.
    pub format: Option<u32>,
    /// Its id; the file's name in the mod's namespace by default.
    pub id: Option<String>,
    pub name: Option<Text>,
    pub tooltip: Option<Text>,
    pub register: Option<Register>,
    /// Mods it needs — Yog or Java mod ids (`pehkui`): without every one of
    /// them loaded it isn't there (an optional integration).
    #[data(default)]
    pub requires: Vec<String>,
    /// Item tags it's in: Yog's common tags (`yog:planks`, `yog:ingots/iron`
    /// — the game's or each loader's tag of that meaning), the game's
    /// (`minecraft:planks`), or the mod's own (a bare name).
    #[data(default)]
    pub tags: Vec<String>,
    /// Items need a model; potion-only `.yitem` files don't register one.
    pub model: Option<Model>,
    pub item: Option<ItemProps>,
}

/// A `.yblock` — the block and its item form.
#[derive(Schema, Debug, Clone)]
pub struct BlockFile {
    pub format: Option<u32>,
    pub id: Option<String>,
    pub name: Option<Text>,
    pub tooltip: Option<Text>,
    pub register: Option<Register>,
    /// Mods it needs — Yog or Java mod ids (`pehkui`): without every one of
    /// them loaded it isn't there (an optional integration).
    #[data(default)]
    pub requires: Vec<String>,
    /// Tags it's in (`yog:logs` — Yog's common tags, `minecraft:logs`,
    /// `edified_logs` — a bare name is the mod's own): the block tag, and the
    /// item tag of the same name for its item form, as the game pairs them.
    /// A tag of blocks only is a `.ydef`.
    #[data(default)]
    pub tags: Vec<String>,
    /// One of the game's kinds of block — `stairs`, `slab`, `door`,
    /// `trapdoor`, `button`, `pressure_plate`, `fence`, `fence_gate`: it
    /// behaves as the game's, its states and models are the kind's, and
    /// `model` gives only `textures` (the kind's slots).
    #[data(with = "Opt<KindRef>")]
    pub kind: Option<String>,
    pub model: Model,
    /// The item form's model; the block's by default.
    pub item_model: Option<Model>,
    /// The item form: how many stack, whether it survives fire, how long it
    /// burns as fuel.
    pub item: Option<BlockItemProps>,
    pub block: Option<BlockProps>,
    pub states: Option<States>,
}
