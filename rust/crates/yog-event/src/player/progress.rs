//! What players achieve.

use yog_data::Schema;

/// Fired when a player earns an advancement (Post only).
#[derive(Debug, Clone, Schema)]
pub struct Advancement {
    pub player_name: String,
    pub uuid: String,
    /// Namespaced id of the advancement, e.g. `"minecraft:story/mine_stone"`.
    pub advancement_id: String,
    pub dimension: String,
}
