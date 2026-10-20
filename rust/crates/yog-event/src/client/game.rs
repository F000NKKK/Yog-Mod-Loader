//! The game running on the client.

use yog_data::Schema;

/// Fired every client tick on the render thread.
#[derive(Debug, Clone, Schema)]
pub struct Tick {}
