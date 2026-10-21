//! Client-side helpers (ABI minor 1): opening a screen with flags, sounds
//! only the local player hears, the local player's held items and the
//! keyboard state. All are no-ops (or `None`/`false`) on a dedicated server
//! and before the mod registered.
//!
//! ```ignore
//! use yog_api::client::{self, ui_flags};
//! client::open_ui_flags("mymod:overlay", ui_flags::MODAL | ui_flags::NO_BACKGROUND);
//! let loop_ = client::play_sound("mymod:hum", 1.0, 1.0, true);
//! client::stop_sound(loop_);
//! ```

use yog_event::client;
pub use yog_event::client::*;
pub use yog_event::ui::flags as ui_flags;

use crate::call;

/// Open the Yog UI `ui_id` with [`ui_flags`] bits.
pub fn open_ui_flags(ui_id: &str, flags: u32) {
    call(&yog_event::ui::OpenWithFlags {
        ui_id: ui_id.to_owned(),
        flags: i32::try_from(flags).unwrap_or(i32::MAX),
    });
}

/// A sound started by [`play_sound`]; `SoundHandle(0)` is "none".
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct SoundHandle(pub u32);

/// Play sound event `sound_id` for the local player only, at their
/// position; `looping` keeps it playing (following the player) until
/// [`stop_sound`].
pub fn play_sound(sound_id: &str, volume: f32, pitch: f32, looping: bool) -> SoundHandle {
    let handle = call(&client::PlaySound {
        sound: sound_id.to_owned(),
        volume,
        pitch,
        looping,
    })
    .flatten();
    SoundHandle(handle.and_then(|h| u32::try_from(h).ok()).unwrap_or(0))
}

/// Stop a sound from [`play_sound`].
pub fn stop_sound(handle: SoundHandle) {
    if handle.0 == 0 {
        return;
    }
    call(&client::StopSound {
        handle: i32::try_from(handle.0).unwrap_or(i32::MAX),
    });
}

/// Tints the block at `pos` in `dimension` on this client — the faces of
/// its model with a `tint` index — `0xRRGGBB`; `None` clears it. The mod
/// sends what to tint (its own packet); the tint lasts till cleared or the
/// game closes.
pub fn set_block_tint(dimension: &str, pos: yog_core::BlockPos, color: Option<u32>) {
    /// What the host reads as "no tint".
    const CLEAR: i32 = -1;
    let color = color.map_or(CLEAR, |c| (c & 0xFF_FFFF) as i32);
    call(&client::SetBlockTint {
        dim: dimension.to_owned(),
        pos,
        color,
    });
}

/// GLFW key codes for [`key_down`].
pub mod keys {
    /// No key: a key binding left unbound (GLFW's `GLFW_KEY_UNKNOWN`).
    pub const UNBOUND: i32 = -1;
    /// Letters: GLFW codes them as their capital's ASCII.
    pub const A: i32 = 65;
    pub const B: i32 = 66;
    pub const C: i32 = 67;
    pub const D: i32 = 68;
    pub const E: i32 = 69;
    pub const F: i32 = 70;
    pub const G: i32 = 71;
    pub const H: i32 = 72;
    pub const I: i32 = 73;
    pub const J: i32 = 74;
    pub const K: i32 = 75;
    pub const L: i32 = 76;
    pub const M: i32 = 77;
    pub const N: i32 = 78;
    pub const O: i32 = 79;
    pub const P: i32 = 80;
    pub const Q: i32 = 81;
    pub const R: i32 = 82;
    pub const S: i32 = 83;
    pub const T: i32 = 84;
    pub const U: i32 = 85;
    pub const V: i32 = 86;
    pub const W: i32 = 87;
    pub const X: i32 = 88;
    pub const Y: i32 = 89;
    pub const Z: i32 = 90;
    pub const HOME: i32 = 268;
    pub const LEFT_SHIFT: i32 = 340;
    pub const LEFT_CONTROL: i32 = 341;
    pub const LEFT_ALT: i32 = 342;
    pub const RIGHT_SHIFT: i32 = 344;
    pub const RIGHT_CONTROL: i32 = 345;
    pub const RIGHT_ALT: i32 = 346;
}

/// Either Control key is held (Minecraft's `Screen.hasControlDown()`; on
/// macOS Minecraft uses Command — not covered here).
pub fn control_down() -> bool {
    key_down(keys::LEFT_CONTROL) || key_down(keys::RIGHT_CONTROL)
}
