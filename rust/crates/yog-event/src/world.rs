//! What happens to the world.

use yog_data::Schema;

pub mod generation;
pub mod loot;

/// Fired when an explosion occurs in a world.
///
/// - `Pre`  — fires before block destruction; return `false` to cancel
///   (blocks and entities are unaffected).
/// - `Post` — fires after the explosion has taken effect.
#[derive(Debug, Clone, Schema)]
pub struct Explosion {
    pub dimension: String,
    pub x: f64,
    pub y: f64,
    pub z: f64,
    pub power: f32,
    /// UUID of the entity that caused the explosion, or empty string if none.
    pub cause_uuid: String,
}
