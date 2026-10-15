//! Client-side primitives of ABI minor 1: opening a screen with flags,
//! local sounds, the local player's held items and key state. Each call
//! goes to a static method of `dev.yog.YogClientApi` (one per platform);
//! on a dedicated server that class does nothing.

use std::os::raw::c_void;

use yog_abi::{YogOwnedStr, YogStr};

use super::owned;

pub(crate) unsafe extern "C" fn api_ui_open_flags(_ctx: *mut c_void, ui_id: YogStr, flags: u32) {
    unsafe {
        if ui_id.as_str().is_empty() {
            return;
        }
        let _: Option<()> = host!(
            "dev/yog/YogClientApi",
            "openUI",
            [S(ui_id), I(flags as i32)]
        );
    }
}

pub(crate) unsafe extern "C" fn api_client_play_sound(
    _ctx: *mut c_void,
    sound_id: YogStr,
    volume: f32,
    pitch: f32,
    looping: bool,
) -> u32 {
    unsafe {
        let handle: Option<i32> = host!(
            "dev/yog/YogClientApi",
            "playSound",
            [S(sound_id), F(volume), F(pitch), Z(looping)]
        );
        handle.map_or(0, |h| h.max(0) as u32)
    }
}

pub(crate) unsafe extern "C" fn api_client_stop_sound(_ctx: *mut c_void, handle: u32) {
    unsafe {
        if handle == 0 {
            return;
        }
        let _: Option<()> = host!("dev/yog/YogClientApi", "stopSound", [I(handle as i32)]);
    }
}

pub(crate) unsafe extern "C" fn api_client_held_item(
    _ctx: *mut c_void,
    off_hand: bool,
) -> YogOwnedStr {
    unsafe { owned(host!("dev/yog/YogClientApi", "heldItem", [Z(off_hand)])) }
}

pub(crate) unsafe extern "C" fn api_client_key_down(_ctx: *mut c_void, key: i32) -> bool {
    unsafe { host!("dev/yog/YogClientApi", "keyDown", [I(key)]).unwrap_or(false) }
}

pub(crate) unsafe extern "C" fn api_client_set_block_tint(
    _ctx: *mut c_void,
    dim: YogStr,
    pos: yog_abi::YogBlockPos,
    color: i64,
) {
    unsafe {
        let _: Option<()> = host!(
            "dev/yog/YogClientApi",
            "setBlockTint",
            [S(dim), I(pos.x), I(pos.y), I(pos.z), I(color.clamp(-1, 0xFF_FFFF) as i32)]
        );
    }
}

pub(crate) unsafe extern "C" fn api_client_set_glider(_ctx: *mut c_void, uuid: YogStr, glide: bool, wings: YogStr) {
    unsafe {
        let _: Option<()> = host!("dev/yog/YogClientApi", "setGlider", [S(uuid), Z(glide), S(wings)]);
    }
}
