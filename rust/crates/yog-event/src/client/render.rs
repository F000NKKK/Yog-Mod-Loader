//! What the client draws.

use yog_data::Schema;
use yog_data::with::Raw;

/// Fired every frame when the HUD is rendered.
/// `delta_tick` is the partial-tick interpolation factor (0.0–1.0).
#[derive(Debug, Clone, Schema)]
pub struct Hud {
    pub delta_tick: f32,
}

/// Fired every frame at the end of world rendering (the frame's graphics
/// carry the camera: `view_proj`, `camera_pos`).
#[derive(Debug, Clone, Schema)]
pub struct World {
    pub delta_tick: f32,
}

/// A mod's screen drawn this frame (after its background); keyed by the screen's id.
#[derive(Debug, Clone, Schema)]
pub struct Ui {
    pub ui_id: String,
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

/// A stack's tooltip picture drawn in `rect` (`[x, y, w, h]`, screen
/// pixels); keyed by the item (`item::Picture` sized it).
#[derive(Debug, Clone, Schema)]
pub struct Tooltip {
    pub item: String,
    pub snbt: String,
    pub rect: [f32; 4],
}

/// The picture of a book page of type `drawn` drawn in `rect` (`[x, y, w,
/// h]`, screen pixels), every frame the page is open; keyed by its `drawer`.
#[derive(Debug, Clone, Schema)]
pub struct Page {
    pub drawer: String,
    /// The page's `data`.
    pub data: yog_data::Value,
    pub rect: [f32; 4],
}
