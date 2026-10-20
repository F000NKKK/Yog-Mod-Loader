//! The game running on the server.

use yog_data::Schema;

/// Fired every server tick (20 a second).
#[derive(Debug, Clone, Schema)]
pub struct Tick {}
