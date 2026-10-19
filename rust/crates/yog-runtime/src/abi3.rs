//! ABI minor 3 additions.

use super::*;

// ── ABI minor 3 ──────────────────────────────────────────────────────────────

pub(super) unsafe extern "C" fn srv_set_item_cooldown(_ctx: *mut c_void, player: YogStr, item_id: YogStr, ticks: i32) -> bool {
    unsafe { bridge!("setItemCooldown", [S(player), S(item_id), I(ticks)]).unwrap_or(false) }
}

pub(super) unsafe extern "C" fn srv_entity_vehicle(_ctx: *mut c_void, uuid: YogStr) -> YogOwnedStr {
    unsafe { owned(bridge!("entityVehicle", [S(uuid)])) }
}

pub(super) unsafe extern "C" fn srv_entity_passengers(_ctx: *mut c_void, uuid: YogStr) -> YogOwnedStr {
    unsafe { owned(bridge!("entityPassengers", [S(uuid)])) }
}

pub(super) unsafe extern "C" fn srv_entity_mount(_ctx: *mut c_void, rider: YogStr, vehicle: YogStr, force: bool) -> bool {
    unsafe { bridge!("entityMount", [S(rider), S(vehicle), Z(force)]).unwrap_or(false) }
}

pub(super) unsafe extern "C" fn srv_entity_dismount(_ctx: *mut c_void, uuid: YogStr) -> bool {
    unsafe { bridge!("entityDismount", [S(uuid)]).unwrap_or(false) }
}

/// The server's hitbox now, the clients' through entity data.
pub(super) unsafe extern "C" fn srv_entity_set_dimensions(_ctx: *mut c_void, uuid: YogStr, w: f32, h: f32, eye: f32) -> bool {
    let found: bool = unsafe { bridge!("entitySetDimensions", [S(uuid), F(w), F(h), F(eye)]).unwrap_or(false) };
    if found {
        let value = (w >= 0.0).then(|| entity_data::encode_dims(w, h, eye));
        entity_data::set(unsafe { uuid.as_str() }, entity_data::DIMENSIONS, value.as_deref(), false);
    }
    found
}

pub(super) unsafe extern "C" fn srv_entity_set_fall_distance(_ctx: *mut c_void, uuid: YogStr, distance: f32) -> bool {
    unsafe { bridge!("entitySetFallDistance", [S(uuid), F(distance)]).unwrap_or(false) }
}

pub(super) unsafe extern "C" fn srv_redstone_signal(_ctx: *mut c_void, dim: YogStr, pos: YogBlockPos) -> i32 {
    unsafe { bridge!("redstoneSignal", [S(dim), I(pos.x), I(pos.y), I(pos.z)]).unwrap_or(-1) }
}

pub(super) unsafe extern "C" fn srv_redstone_power(_ctx: *mut c_void, dim: YogStr, pos: YogBlockPos) -> i32 {
    unsafe { bridge!("redstonePower", [S(dim), I(pos.x), I(pos.y), I(pos.z)]).unwrap_or(-1) }
}

pub(super) unsafe extern "C" fn srv_set_redstone_output(_ctx: *mut c_void, dim: YogStr, pos: YogBlockPos, level: i32) -> bool {
    unsafe { bridge!("setRedstoneOutput", [S(dim), I(pos.x), I(pos.y), I(pos.z), I(level)]).unwrap_or(false) }
}

pub(super) unsafe extern "C" fn srv_get_block_state(_ctx: *mut c_void, dim: YogStr, pos: YogBlockPos) -> YogOwnedStr {
    unsafe { owned(bridge!("getBlockState", [S(dim), I(pos.x), I(pos.y), I(pos.z)])) }
}

pub(super) unsafe extern "C" fn srv_set_block_state(_ctx: *mut c_void, dim: YogStr, pos: YogBlockPos, state: YogStr) -> bool {
    unsafe { bridge!("setBlockState", [S(dim), I(pos.x), I(pos.y), I(pos.z), S(state)]).unwrap_or(false) }
}

pub(super) unsafe extern "C" fn srv_world_is_thundering(_ctx: *mut c_void, dim: YogStr) -> bool {
    unsafe { bridge!("worldIsThundering", [S(dim)]).unwrap_or(false) }
}

pub(super) unsafe extern "C" fn srv_world_set_thunder(_ctx: *mut c_void, dim: YogStr, thundering: bool, duration_ticks: i32) -> bool {
    unsafe { bridge!("worldSetThunder", [S(dim), Z(thundering), I(duration_ticks)]).unwrap_or(false) }
}

pub(super) unsafe extern "C" fn srv_fill_region(
    _ctx: *mut c_void,
    dim: YogStr,
    from: YogBlockPos,
    to: YogBlockPos,
    state: YogStr,
) -> i32 {
    unsafe {
        bridge!(
            "fillRegion",
            [S(dim), I(from.x), I(from.y), I(from.z), I(to.x), I(to.y), I(to.z), S(state)]
        )
        .unwrap_or(-1)
    }
}

pub(super) unsafe extern "C" fn srv_scan_region(_ctx: *mut c_void, dim: YogStr, from: YogBlockPos, to: YogBlockPos) -> YogOwnedStr {
    unsafe { owned(bridge!("scanRegion", [S(dim), I(from.x), I(from.y), I(from.z), I(to.x), I(to.y), I(to.z)])) }
}
