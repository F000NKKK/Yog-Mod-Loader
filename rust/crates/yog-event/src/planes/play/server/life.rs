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

/// Fired as the world is saved — an autosave, `/save-all`, the server
/// stopping — before its chunks are written: what's kept with the world is
/// written now, so it's saved with the world it belongs to.
#[derive(Debug, Clone, Schema)]
pub struct Saving {}
