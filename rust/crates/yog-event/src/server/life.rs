//! The server starting and stopping.

use yog_data::Schema;

/// Fired once the server has started.
#[derive(Debug, Clone, Schema)]
pub struct Started {
    /// The world's folder.
    pub world_dir: String,
}

/// Fired as the server stops.
#[derive(Debug, Clone, Schema)]
pub struct Stopping {}
