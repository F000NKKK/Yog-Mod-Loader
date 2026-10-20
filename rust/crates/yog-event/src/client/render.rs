//! What the client draws.

use yog_data::Schema;

/// Fired every frame when the HUD is rendered.
/// `delta_tick` is the partial-tick interpolation factor (0.0–1.0).
#[derive(Debug, Clone, Schema)]
pub struct Hud {
    pub delta_tick: f32,
}
