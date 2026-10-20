//! What mods keep on entities, and their sizes: services the runtime
//! provides (it keeps the data and syncs it to the clients that see them).

use yog_data::Schema;
use yog_data::with::Raw;

/// Bytes a mod keeps on an entity under a key.
#[derive(Debug, Clone, PartialEq, Schema)]
pub struct DataValue {
    #[data(with = "Raw")]
    pub bytes: Vec<u8>,
}

/// What the entity keeps under `key`.
#[derive(Debug, Clone, Schema)]
pub struct Data {
    pub uuid: String,
    pub key: String,
}

/// Keeps `value` under `key` (`None` forgets it), synced to the clients
/// that see the entity; `persist` saves it with the entity.
#[derive(Debug, Clone, Schema)]
pub struct SetData {
    pub uuid: String,
    pub key: String,
    pub value: Option<DataValue>,
    pub persist: bool,
}

/// What the host reads as "vanilla's size again": a negative width.
pub const VANILLA_SIZE: [f32; 3] = [-1.0, 0.0, 0.0];

/// The entity's size, `[width, height, eye height]` — `None`: vanilla's —
/// on the server and on every client that sees it.
#[derive(Debug, Clone, Schema)]
pub struct SetDimensions {
    pub uuid: String,
    pub size: Option<[f32; 3]>,
}

/// How a hanging entity hangs: on which side, how big, and the item it was
/// placed from (what it drops) — its entity data `yog:hanging`
/// (`yog_ids::yog::HANGING`), which the host keeps up to date.
#[derive(Debug, Clone, PartialEq, Eq, Schema)]
pub struct Hanging {
    /// The way it faces, out of its wall: `north`, `south`, `west`, `east`.
    pub facing: String,
    /// Whole blocks of wall, wide and high.
    pub width: u32,
    pub height: u32,
    /// The item it keeps: its id and custom data (SNBT, `{}` for none).
    pub item: String,
    pub nbt: String,
}
