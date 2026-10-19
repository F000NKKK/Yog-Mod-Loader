//! Mods' logging (ABI 3.8).

use super::*;

// ── Logging (ABI 3.8) ────────────────────────────────────────────────────────

thread_local! {
    /// Set while a record is on its way to Java: a record the call itself
    /// logs (a JNI failure) goes to stderr instead of looping.
    static LOGGING: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

/// Writes a record to the game's log (`YogLog.log`); before there's a JVM, or from inside
/// a failing log call, to stderr.
pub(super) fn host_log(level: yog_logging::Level, message: &str) {
    let sent = !LOGGING.with(|busy| busy.replace(true)) && {
        let r: Option<()> = unsafe { host!("dev/yog/YogLog", "log", [JArg::I(level as i32), JArg::S(YogStr::from_str(message))]) };
        LOGGING.with(|busy| busy.set(false));
        r.is_some()
    };
    if !sent {
        eprintln!("[yog] [{level:?}] {message}");
    }
}

/// `YogApi::log`: a mod's record.
pub(super) unsafe extern "C" fn api_log(level: u32, message: YogStr) {
    host_log(yog_logging::Level::from_u32(level), unsafe { message.as_str() });
}

pub(super) unsafe extern "C" fn api_client_advancement_done(_ctx: *mut c_void, id: YogStr) -> bool {
    client_advancement_done(unsafe { id.as_str() })
}

pub(super) unsafe extern "C" fn srv_entity_attribute_get(
    _ctx: *mut c_void,
    uuid: YogStr,
    attr: YogStr,
) -> f64 {
    unsafe { bridge!("entityAttributeGet", [S(uuid), S(attr)]).unwrap_or(f64::NAN) }
}

pub(super) unsafe extern "C" fn srv_entity_attribute_set(
    _ctx: *mut c_void,
    uuid: YogStr,
    attr: YogStr,
    value: f64,
) -> bool {
    unsafe { bridge!("entityAttributeSet", [S(uuid), S(attr), D(value)]).unwrap_or(false) }
}

pub(super) unsafe extern "C" fn srv_get_held_item_nbt(_ctx: *mut c_void, player: YogStr) -> YogOwnedStr {
    unsafe { owned(bridge!("getHeldItemNbt", [S(player)])) }
}

pub(super) unsafe extern "C" fn srv_set_held_item_nbt(
    _ctx: *mut c_void,
    player: YogStr,
    snbt: YogStr,
) -> bool {
    unsafe { bridge!("setHeldItemNbt", [S(player), S(snbt)]).unwrap_or(false) }
}

pub(super) unsafe extern "C" fn srv_get_offhand_item_nbt(_ctx: *mut c_void, player: YogStr) -> YogOwnedStr {
    unsafe { owned(bridge!("getOffhandItemNbt", [S(player)])) }
}

pub(super) unsafe extern "C" fn srv_set_offhand_item_nbt(
    _ctx: *mut c_void,
    player: YogStr,
    snbt: YogStr,
) -> bool {
    unsafe { bridge!("setOffhandItemNbt", [S(player), S(snbt)]).unwrap_or(false) }
}

pub(super) unsafe extern "C" fn srv_get_slot_item(
    _ctx: *mut c_void,
    player: YogStr,
    slot: u32,
) -> YogOwnedStr {
    unsafe { owned(bridge!("getSlotItem", [S(player), I(slot as i32)])) }
}

pub(super) unsafe extern "C" fn srv_set_slot_item(
    _ctx: *mut c_void,
    player: YogStr,
    slot: u32,
    item_id: YogStr,
    count: u32,
    snbt: YogStr,
) -> bool {
    unsafe {
        bridge!(
            "setSlotItem",
            [
                S(player),
                I(slot as i32),
                S(item_id),
                I(count as i32),
                S(snbt)
            ]
        )
        .unwrap_or(false)
    }
}
