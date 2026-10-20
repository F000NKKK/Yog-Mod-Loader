//! What the player presses.

use yog_data::Schema;

/// Fired on every key press, release, or repeat (client-side).
///
/// Return `false` in the handler to prevent Minecraft from processing the key.
#[derive(Debug, Clone, Schema)]
pub struct Key {
    /// GLFW key code (e.g. 69 = E). See `org.lwjgl.glfw.GLFW`.
    pub key_code: i32,
    pub scan_code: i32,
    /// 0 = release, 1 = press, 2 = repeat.
    pub action: i32,
    /// Modifier bitmask: 1=Shift, 2=Ctrl, 4=Alt, 8=Super.
    pub modifiers: i32,
}
