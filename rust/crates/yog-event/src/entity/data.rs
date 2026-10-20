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

/// The entity's size, `[width, height, eye height]` — `None`: vanilla's —
/// on the server and on every client that sees it.
#[derive(Debug, Clone, Schema)]
pub struct SetDimensions {
    pub uuid: String,
    pub size: Option<[f32; 3]>,
}
