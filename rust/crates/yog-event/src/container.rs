//! Containers opened and closed.

use yog_data::Schema;

pub mod machine;

/// Fired when a player opens a container screen.
///
/// - `Pre`  — return `false` to prevent the screen from opening.
/// - `Post` — screen opened; `container_type` is set.
#[derive(Debug, Clone, Schema)]
pub struct Open {
    pub player_name: String,
    pub player_uuid: String,
    /// Screen handler registry id, e.g. `"minecraft:chest"`.
    /// Empty string for screens not in the registry (e.g. the player inventory).
    pub container_type: String,
    pub dimension: String,
}

/// Fired when a player closes a container screen (Post only).
#[derive(Debug, Clone, Schema)]
pub struct Close {
    pub player_name: String,
    pub player_uuid: String,
    pub dimension: String,
}
