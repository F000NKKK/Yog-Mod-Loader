//! The camera: where the local player's view is and how it turns, changed
//! by mods each frame (after vanilla placed it).

use crate::math::Vec3;

/// The camera as vanilla placed it this frame.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct CameraFrame {
    /// The entity the camera follows.
    pub uuid: String,
    pub partial: f32,
    /// World position of the camera.
    pub pos: [f64; 3],
    /// Degrees.
    pub yaw: f32,
    pub pitch: f32,
    /// Third person (behind or in front).
    pub detached: bool,
    /// `entity_flags` bits of the entity.
    pub flags: u32,
}

/// What mods change; each handler gets the last one's result and adds to it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CameraOut {
    /// Blocks, along the camera's own axes: right, up, forward.
    pub local_offset: Vec3,
    /// Blocks, in the world.
    pub world_offset: Vec3,
    /// Degrees added to the view.
    pub yaw: f32,
    pub pitch: f32,
    /// Tilt about the view direction.
    pub roll: f32,
    /// Multiplies the field of view.
    pub fov: f32,
}

impl Default for CameraOut {
    fn default() -> Self {
        Self { local_offset: Vec3::ZERO, world_offset: Vec3::ZERO, yaw: 0.0, pitch: 0.0, roll: 0.0, fov: 1.0 }
    }
}
