//! Players dying and coming back.

use yog_data::Schema;

/// Fired when a player dies.
///
/// - `Pre`  — fires before death is processed; return `false` to prevent death
///   (entity survives at 0.5 HP).
/// - `Post` — fires after the player has died.
#[derive(Debug, Clone, Schema)]
pub struct Death {
    pub player_name: String,
    pub uuid: String,
    /// Damage source identifier, e.g. `"player"`, `"fall"`.
    pub source: String,
    pub dimension: String,
}

/// Fired when a player respawns after death (Post only).
#[derive(Debug, Clone, Schema)]
pub struct Respawn {
    pub player_name: String,
    pub uuid: String,
    /// True if respawning at a bed or respawn anchor; false for world spawn.
    pub at_anchor: bool,
    /// Dimension respawned into (may differ from where the player died).
    pub dimension: String,
}
