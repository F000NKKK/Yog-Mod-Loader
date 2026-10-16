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

use yog_abi::YogStr;
pub use yog_abi::ui_flags;

use crate::registry::global_api;

/// Open the Yog UI `ui_id` with [`ui_flags`] bits.
pub fn open_ui_flags(ui_id: &str, flags: u32) {
    let api = global_api();
    if api.is_null() {
        return;
    }
    unsafe { ((*api).ui_open_flags)((*api).ctx, YogStr::from_str(ui_id), flags) }
}

/// A sound started by [`play_sound`]; `SoundHandle(0)` is "none".
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct SoundHandle(pub u32);

/// Play sound event `sound_id` for the local player only, at their
/// position; `looping` keeps it playing (following the player) until
/// [`stop_sound`].
pub fn play_sound(sound_id: &str, volume: f32, pitch: f32, looping: bool) -> SoundHandle {
    let api = global_api();
    if api.is_null() {
        return SoundHandle(0);
    }
    SoundHandle(unsafe {
        ((*api).client_play_sound)(
            (*api).ctx,
            YogStr::from_str(sound_id),
            volume,
            pitch,
            looping,
        )
    })
}

/// Stop a sound from [`play_sound`].
pub fn stop_sound(handle: SoundHandle) {
    let api = global_api();
    if api.is_null() || handle.0 == 0 {
        return;
    }
    unsafe { ((*api).client_stop_sound)((*api).ctx, handle.0) }
}

/// Item id in the local player's main hand (or `off_hand`), `None` for an
/// empty hand.
pub fn held_item(off_hand: bool) -> Option<String> {
    let api = global_api();
    if api.is_null() {
        return None;
    }
    let owned = unsafe { ((*api).client_held_item)((*api).ctx, off_hand) };
    crate::registry::take_owned(unsafe { (*api).free_str }, owned)
}

/// Attribute `attribute_id`'s value on this client's player — its base
/// changed by every modifier (effects, equipment); `None` without a player
/// or such an attribute.
pub fn attribute_value(attribute_id: &str) -> Option<f64> {
    let api = global_api();
    if api.is_null() {
        return None;
    }
    let v = unsafe { ((*api).client_attribute_value)((*api).ctx, YogStr::from_str(attribute_id)) };
    (!v.is_nan()).then_some(v)
}

/// Tints the block at `pos` in `dimension` on this client — the faces of
/// its model with a `tint` index — `0xRRGGBB`; `None` clears it. The mod
/// sends what to tint (its own packet); the tint lasts till cleared or the
/// game closes.
pub fn set_block_tint(dimension: &str, pos: yog_core::BlockPos, color: Option<u32>) {
    let api = global_api();
    if api.is_null() {
        return;
    }
    let p = yog_abi::YogBlockPos { x: pos.x, y: pos.y, z: pos.z };
    let c = color.map_or(-1, |c| (c & 0xFF_FFFF) as i64);
    unsafe { ((*api).client_set_block_tint)((*api).ctx, YogStr::from_str(dimension), p, c) }
}

/// GLFW key codes for [`key_down`].
pub mod keys {
    pub const LEFT_SHIFT: i32 = 340;
    pub const LEFT_CONTROL: i32 = 341;
    pub const LEFT_ALT: i32 = 342;
    pub const RIGHT_SHIFT: i32 = 344;
    pub const RIGHT_CONTROL: i32 = 345;
    pub const RIGHT_ALT: i32 = 346;
}

/// Whether GLFW key `key` is held right now.
pub fn key_down(key: i32) -> bool {
    let api = global_api();
    if api.is_null() {
        return false;
    }
    unsafe { ((*api).client_key_down)((*api).ctx, key) }
}

/// Either Control key is held (Minecraft's `Screen.hasControlDown()`; on
/// macOS Minecraft uses Command — not covered here).
pub fn control_down() -> bool {
    key_down(keys::LEFT_CONTROL) || key_down(keys::RIGHT_CONTROL)
}
