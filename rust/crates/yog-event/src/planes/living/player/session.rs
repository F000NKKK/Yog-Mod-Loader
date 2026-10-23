//! Players joining and leaving.

use yog_data::Schema;

/// Fired when a player joins the server.
#[derive(Debug, Clone, Schema)]
pub struct Join {
    pub player_name: String,
    /// Player UUID as a string, e.g. `069a79f4-44e9-4726-a5be-fca90e38aaf5`.
    pub uuid: String,
    pub dimension: String,
}

/// Fired when a player leaves the server.
#[derive(Debug, Clone, Schema)]
pub struct Leave {
    pub player_name: String,
    pub uuid: String,
    pub dimension: String,
}
