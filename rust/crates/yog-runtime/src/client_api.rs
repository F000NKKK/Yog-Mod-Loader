//! Client-side primitives of ABI minor 1: opening a screen with flags,
//! local sounds, the local player's held items and key state. Each call
//! goes to a static method of `dev.yog.YogClientApi` (one per platform);
//! on a dedicated server that class does nothing.

use std::os::raw::c_void;

use jni::objects::{JObject, JValue};
use yog_abi::{YogOwnedStr, YogStr};

use super::get_env;

const CLASS: &str = "dev/yog/YogClientApi";

pub(crate) unsafe extern "C" fn api_ui_open_flags(_ctx: *mut c_void, ui_id: YogStr, flags: u32) {
    let id = ui_id.as_str().to_string();
    if id.is_empty() {
        return;
    }
    let Some(mut env) = get_env() else { return };
    let Ok(jid) = env.new_string(&id) else { return };
    let jid: JObject = jid.into();
    if env
        .call_static_method(
            CLASS,
            "openUI",
            "(Ljava/lang/String;I)V",
            &[JValue::Object(&jid), JValue::Int(flags as i32)],
        )
        .is_err()
    {
        env.exception_clear().ok();
    }
    let _ = env.delete_local_ref(jid);
}

pub(crate) unsafe extern "C" fn api_client_play_sound(
    _ctx: *mut c_void,
    sound_id: YogStr,
    volume: f32,
    pitch: f32,
    looping: bool,
) -> u32 {
    let Some(mut env) = get_env() else { return 0 };
    let Ok(jid) = env.new_string(sound_id.as_str()) else {
        return 0;
    };
    let jid: JObject = jid.into();
    let r = env.call_static_method(
        CLASS,
        "playSound",
        "(Ljava/lang/String;FFZ)I",
        &[
            JValue::Object(&jid),
            JValue::Float(volume),
            JValue::Float(pitch),
            JValue::Bool(looping as u8),
        ],
    );
    let _ = env.delete_local_ref(jid);
    match r.and_then(|v| v.i()) {
        Ok(h) => h.max(0) as u32,
        Err(_) => {
            env.exception_clear().ok();
            0
        }
    }
}

pub(crate) unsafe extern "C" fn api_client_stop_sound(_ctx: *mut c_void, handle: u32) {
    if handle == 0 {
        return;
    }
    let Some(mut env) = get_env() else { return };
    if env
        .call_static_method(CLASS, "stopSound", "(I)V", &[JValue::Int(handle as i32)])
        .is_err()
    {
        env.exception_clear().ok();
    }
}

pub(crate) unsafe extern "C" fn api_client_held_item(
    _ctx: *mut c_void,
    off_hand: bool,
) -> YogOwnedStr {
    let Some(mut env) = get_env() else {
        return YogOwnedStr::NONE;
    };
    let r = env.call_static_method(
        CLASS,
        "heldItem",
        "(Z)Ljava/lang/String;",
        &[JValue::Bool(off_hand as u8)],
    );
    match r.and_then(|v| v.l()) {
        Ok(obj) if !obj.as_raw().is_null() => {
            let s: Option<String> = env.get_string(&obj.into()).ok().map(String::from);
            s.map_or(YogOwnedStr::NONE, YogOwnedStr::from_string)
        }
        Ok(_) => YogOwnedStr::NONE,
        Err(_) => {
            env.exception_clear().ok();
            YogOwnedStr::NONE
        }
    }
}

pub(crate) unsafe extern "C" fn api_client_key_down(_ctx: *mut c_void, key: i32) -> bool {
    let Some(mut env) = get_env() else {
        return false;
    };
    match env
        .call_static_method(CLASS, "keyDown", "(I)Z", &[JValue::Int(key)])
        .and_then(|v| v.z())
    {
        Ok(b) => b,
        Err(_) => {
            env.exception_clear().ok();
            false
        }
    }
}
