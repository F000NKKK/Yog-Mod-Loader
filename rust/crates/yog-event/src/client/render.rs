//! What the client draws.

use yog_data::Schema;
use yog_data::with::Raw;

/// Fired every frame when the HUD is rendered.
/// `delta_tick` is the partial-tick interpolation factor (0.0–1.0).
#[derive(Debug, Clone, Schema)]
pub struct Hud {
    pub delta_tick: f32,
}

/// The camera this frame, as vanilla placed it, and what the mods make of
/// it: each handler adds to `out`, seeing the one before's.
#[derive(Debug, Clone, Schema)]
pub struct Camera {
    pub frame: yog_body::CameraFrame,
    pub out: yog_body::CameraOut,
}

/// A tracked entity's body this frame (`yog_body::wire`): each handler
/// appends its ops to `ops` (pose vanilla parts, turn, draw meshes…).
#[derive(Debug, Clone, Schema)]
pub struct Body {
    #[data(with = "Raw")]
    pub frame: Vec<u8>,
    #[data(with = "Raw")]
    pub ops: Vec<u8>,
}
