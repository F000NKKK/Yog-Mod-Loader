//! Entities appearing and dying.

use yog_data::Schema;

/// Fired when any entity is loaded into a world (server side).
///
/// Also used for `on_entity_spawn_pre` (cancellable): return `false` to
/// discard the entity immediately after loading (effective spawn cancellation).
#[derive(Debug, Clone, Schema)]
pub struct Spawn {
    /// Registry id of the entity, e.g. `minecraft:zombie`.
    pub entity_type: String,
    pub uuid: String,
    /// Dimension the entity was added to, e.g. `minecraft:overworld`.
    pub dimension: String,
}

/// Fired after a living entity dies (server side).
#[derive(Debug, Clone, Schema)]
pub struct Death {
    /// Registry id of the entity, e.g. `minecraft:zombie`.
    pub entity_type: String,
    pub uuid: String,
    /// Identifier of the killing damage source, e.g. `minecraft:player`.
    pub source: String,
    pub dimension: String,
}
