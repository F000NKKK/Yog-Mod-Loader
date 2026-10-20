//! The game running on the client.

use yog_data::Schema;

/// Fired every client tick on the render thread.
#[derive(Debug, Clone, Schema)]
pub struct Tick {}

/// The local player's travel step: the first handler to set `velocity`
/// (blocks per tick, drag applied) moves it by that instead of vanilla's.
#[derive(Debug, Clone, Schema)]
pub struct Travel {
    pub frame: yog_body::TravelFrame,
    pub velocity: Option<yog_body::Vec3>,
}
