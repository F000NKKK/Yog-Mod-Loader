//! What players say.

use yog_data::Schema;

/// Fired when a player sends a chat message.
#[derive(Debug, Clone, Schema)]
pub struct Message {
    pub player_name: String,
    pub message: String,
    pub dimension: String,
}
