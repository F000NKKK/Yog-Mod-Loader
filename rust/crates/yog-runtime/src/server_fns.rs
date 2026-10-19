//! `YogServer`'s functions: one per action on the world.

use super::*;

// ── YogServer standalone functions (one per action) ───────────────────────────
//
// ctx is unused here — all state is in the JAVA_VM static.

pub(super) unsafe extern "C" fn srv_broadcast(_ctx: *mut c_void, msg: YogStr) {
    unsafe {
        let _: Option<()> = bridge!("broadcast", [S(msg)]);
    }
}

pub(super) unsafe extern "C" fn srv_get_block(
    _ctx: *mut c_void,
    dim: YogStr,
    pos: YogBlockPos,
) -> YogOwnedStr {
    unsafe { owned(bridge!("getBlock", [S(dim), I(pos.x), I(pos.y), I(pos.z)])) }
}

pub(super) unsafe extern "C" fn srv_set_block(
    _ctx: *mut c_void,
    dim: YogStr,
    pos: YogBlockPos,
    block: YogStr,
) -> bool {
    unsafe {
        bridge!("setBlock", [S(dim), I(pos.x), I(pos.y), I(pos.z), S(block)]).unwrap_or(false)
    }
}

pub(super) unsafe extern "C" fn srv_world_time(_ctx: *mut c_void, dim: YogStr, out: *mut i64) -> bool {
    unsafe {
        match bridge!("worldTime", [S(dim)]) {
            Some(v) if v != i64::MIN => {
                *out = v;
                true
            }
            _ => false,
        }
    }
}

pub(super) unsafe extern "C" fn srv_set_time(_ctx: *mut c_void, dim: YogStr, time: i64) -> bool {
    unsafe { bridge!("worldSetTime", [S(dim), J(time)]).unwrap_or(false) }
}

pub(super) unsafe extern "C" fn srv_is_raining(_ctx: *mut c_void, dim: YogStr) -> bool {
    unsafe { bridge!("worldIsRaining", [S(dim)]).unwrap_or(false) }
}

pub(super) unsafe extern "C" fn srv_set_weather(
    _ctx: *mut c_void,
    dim: YogStr,
    raining: bool,
    dur: i32,
) -> bool {
    unsafe { bridge!("worldSetWeather", [S(dim), Z(raining), I(dur)]).unwrap_or(false) }
}

pub(super) unsafe extern "C" fn srv_give_item(
    _ctx: *mut c_void,
    player: YogStr,
    item: YogStr,
    count: u32,
) -> bool {
    unsafe { bridge!("giveItem", [S(player), S(item), I(count as i32)]).unwrap_or(false) }
}

pub(super) unsafe extern "C" fn srv_player_teleport(_ctx: *mut c_void, player: YogStr, pos: YogVec3) -> bool {
    unsafe { bridge!("teleport", [S(player), D(pos.x), D(pos.y), D(pos.z)]).unwrap_or(false) }
}

pub(super) unsafe extern "C" fn srv_send_to_player(
    _ctx: *mut c_void,
    player: YogStr,
    channel: YogStr,
    data: *const u8,
    len: u32,
) -> bool {
    unsafe {
        bridge!(
            "sendToPlayer",
            [
                S(player),
                S(channel),
                Bytes(std::slice::from_raw_parts(data, len as usize))
            ]
        )
        .unwrap_or(false)
    }
}

pub(super) unsafe extern "C" fn srv_send_to_server(
    _ctx: *mut c_void,
    channel: YogStr,
    data: *const u8,
    len: u32,
) -> bool {
    unsafe {
        host!(
            "dev/yog/YogClient",
            "sendToServer",
            [
                S(channel),
                Bytes(std::slice::from_raw_parts(data, len as usize))
            ]
        )
        .unwrap_or(false)
    }
}

pub(super) unsafe extern "C" fn srv_kick_player(_ctx: *mut c_void, player: YogStr, reason: YogStr) -> bool {
    unsafe { bridge!("kickPlayer", [S(player), S(reason)]).unwrap_or(false) }
}

pub(super) unsafe extern "C" fn srv_set_gamemode(_ctx: *mut c_void, player: YogStr, mode: YogStr) -> bool {
    unsafe { bridge!("setGamemode", [S(player), S(mode)]).unwrap_or(false) }
}

pub(super) unsafe extern "C" fn srv_send_title(
    _ctx: *mut c_void,
    player: YogStr,
    title: YogStr,
    sub: YogStr,
    fi: i32,
    stay: i32,
    fo: i32,
) -> bool {
    unsafe {
        bridge!(
            "sendTitle",
            [S(player), S(title), S(sub), I(fi), I(stay), I(fo)]
        )
        .unwrap_or(false)
    }
}

pub(super) unsafe extern "C" fn srv_send_actionbar(_ctx: *mut c_void, player: YogStr, msg: YogStr) -> bool {
    unsafe { bridge!("sendActionbar", [S(player), S(msg)]).unwrap_or(false) }
}

pub(super) unsafe extern "C" fn srv_play_sound(
    _ctx: *mut c_void,
    dim: YogStr,
    pos: YogVec3,
    sound: YogStr,
    vol: f32,
    pitch: f32,
) -> bool {
    unsafe {
        bridge!(
            "playSound",
            [
                S(dim),
                D(pos.x),
                D(pos.y),
                D(pos.z),
                S(sound),
                F(vol),
                F(pitch)
            ]
        )
        .unwrap_or(false)
    }
}

pub(super) unsafe extern "C" fn srv_play_sound_player(
    _ctx: *mut c_void,
    player: YogStr,
    sound: YogStr,
    vol: f32,
    pitch: f32,
) -> bool {
    unsafe {
        bridge!("playSoundToPlayer", [S(player), S(sound), F(vol), F(pitch)]).unwrap_or(false)
    }
}

pub(super) unsafe extern "C" fn srv_entity_teleport(_ctx: *mut c_void, uuid: YogStr, pos: YogVec3) -> bool {
    unsafe { bridge!("entityTeleport", [S(uuid), D(pos.x), D(pos.y), D(pos.z)]).unwrap_or(false) }
}

pub(super) unsafe extern "C" fn srv_entity_rotation(
    _ctx: *mut c_void,
    uuid: YogStr,
    out: *mut YogVec3,
) -> bool {
    unsafe {
        let Some(s): Option<String> = bridge!("entityRotation", [S(uuid)]) else {
            return false;
        };
        match parse_tsv::<f64, 2>(&s) {
            Some([yaw, pitch]) => {
                *out = YogVec3 {
                    x: yaw,
                    y: pitch,
                    z: 0.0,
                };
                true
            }
            None => false,
        }
    }
}

pub(super) unsafe extern "C" fn srv_entity_position(
    _ctx: *mut c_void,
    uuid: YogStr,
    out: *mut YogVec3,
) -> bool {
    unsafe {
        let Some(s): Option<String> = bridge!("entityPosition", [S(uuid)]) else {
            return false;
        };
        match parse_tsv::<f64, 3>(&s) {
            Some([x, y, z]) => {
                *out = YogVec3 { x, y, z };
                true
            }
            None => false,
        }
    }
}

pub(super) unsafe extern "C" fn srv_entity_health(_ctx: *mut c_void, uuid: YogStr, out: *mut f32) -> bool {
    unsafe {
        match bridge!("entityHealth", [S(uuid)]) {
            Some(v) if !f64::is_nan(v) => {
                *out = v as f32;
                true
            }
            _ => false,
        }
    }
}

pub(super) unsafe extern "C" fn srv_entity_set_health(_ctx: *mut c_void, uuid: YogStr, hp: f32) -> bool {
    unsafe { bridge!("entitySetHealth", [S(uuid), D(hp as f64)]).unwrap_or(false) }
}

pub(super) unsafe extern "C" fn srv_entity_kill(_ctx: *mut c_void, uuid: YogStr) -> bool {
    unsafe { bridge!("entityKill", [S(uuid)]).unwrap_or(false) }
}

pub(super) unsafe extern "C" fn srv_spawn_entity(
    _ctx: *mut c_void,
    type_id: YogStr,
    dim: YogStr,
    pos: YogVec3,
) -> YogOwnedStr {
    unsafe {
        owned(bridge!(
            "spawnEntity",
            [S(type_id), S(dim), D(pos.x), D(pos.y), D(pos.z)]
        ))
    }
}

pub(super) unsafe extern "C" fn srv_entity_add_effect(
    _ctx: *mut c_void,
    uuid: YogStr,
    fx: YogStr,
    dur: i32,
    amp: u8,
    particles: bool,
) -> bool {
    unsafe {
        bridge!(
            "entityAddEffect",
            [S(uuid), S(fx), I(dur), I(amp as i32), Z(particles)]
        )
        .unwrap_or(false)
    }
}

pub(super) unsafe extern "C" fn srv_entity_remove_effect(_ctx: *mut c_void, uuid: YogStr, fx: YogStr) -> bool {
    unsafe { bridge!("entityRemoveEffect", [S(uuid), S(fx)]).unwrap_or(false) }
}

pub(super) unsafe extern "C" fn srv_entity_clear_effects(_ctx: *mut c_void, uuid: YogStr) -> bool {
    unsafe { bridge!("entityClearEffects", [S(uuid)]).unwrap_or(false) }
}

pub(super) unsafe extern "C" fn srv_entity_velocity(
    _ctx: *mut c_void,
    uuid: YogStr,
    out: *mut YogVec3,
) -> bool {
    unsafe {
        let Some(s): Option<String> = bridge!("entityVelocity", [S(uuid)]) else {
            return false;
        };
        match parse_tsv::<f64, 3>(&s) {
            Some([x, y, z]) => {
                *out = YogVec3 { x, y, z };
                true
            }
            None => false,
        }
    }
}

pub(super) unsafe extern "C" fn srv_entity_set_velocity(
    _ctx: *mut c_void,
    uuid: YogStr,
    vel: YogVec3,
) -> bool {
    unsafe {
        bridge!("entitySetVelocity", [S(uuid), D(vel.x), D(vel.y), D(vel.z)]).unwrap_or(false)
    }
}

pub(super) unsafe extern "C" fn srv_entity_add_velocity(
    _ctx: *mut c_void,
    uuid: YogStr,
    vel: YogVec3,
) -> bool {
    unsafe {
        bridge!("entityAddVelocity", [S(uuid), D(vel.x), D(vel.y), D(vel.z)]).unwrap_or(false)
    }
}

pub(super) unsafe extern "C" fn srv_has_item_tag(_ctx: *mut c_void, item: YogStr, tag: YogStr) -> bool {
    unsafe { bridge!("hasItemTag", [S(item), S(tag)]).unwrap_or(false) }
}

pub(super) unsafe extern "C" fn srv_has_block_tag(_ctx: *mut c_void, block: YogStr, tag: YogStr) -> bool {
    unsafe { bridge!("hasBlockTag", [S(block), S(tag)]).unwrap_or(false) }
}

pub(super) unsafe extern "C" fn srv_drop_loot(
    _ctx: *mut c_void,
    table: YogStr,
    dim: YogStr,
    pos: YogVec3,
) -> bool {
    unsafe {
        bridge!("dropLoot", [S(table), S(dim), D(pos.x), D(pos.y), D(pos.z)]).unwrap_or(false)
    }
}

pub(super) unsafe extern "C" fn srv_scoreboard_get(
    _ctx: *mut c_void,
    obj: YogStr,
    player: YogStr,
    out: *mut i32,
) -> bool {
    unsafe {
        match bridge!("scoreboardGet", [S(obj), S(player)]) {
            Some(v) if v != i32::MIN => {
                *out = v;
                true
            }
            _ => false,
        }
    }
}

pub(super) unsafe extern "C" fn srv_scoreboard_set(
    _ctx: *mut c_void,
    obj: YogStr,
    player: YogStr,
    score: i32,
) -> bool {
    unsafe { bridge!("scoreboardSet", [S(obj), S(player), I(score)]).unwrap_or(false) }
}

pub(super) unsafe extern "C" fn srv_scoreboard_add(
    _ctx: *mut c_void,
    obj: YogStr,
    player: YogStr,
    delta: i32,
    out: *mut i32,
) -> bool {
    unsafe {
        match bridge!("scoreboardAdd", [S(obj), S(player), I(delta)]) {
            Some(v) if v != i32::MIN => {
                *out = v;
                true
            }
            _ => false,
        }
    }
}

pub(super) unsafe extern "C" fn srv_bossbar_create(
    _ctx: *mut c_void,
    id: YogStr,
    title: YogStr,
    color: YogStr,
    style: YogStr,
) -> bool {
    unsafe { bridge!("bossbarCreate", [S(id), S(title), S(color), S(style)]).unwrap_or(false) }
}

pub(super) unsafe extern "C" fn srv_bossbar_remove(_ctx: *mut c_void, id: YogStr) -> bool {
    unsafe { bridge!("bossbarRemove", [S(id)]).unwrap_or(false) }
}

pub(super) unsafe extern "C" fn srv_bossbar_set_title(_ctx: *mut c_void, id: YogStr, title: YogStr) -> bool {
    unsafe { bridge!("bossbarSetTitle", [S(id), S(title)]).unwrap_or(false) }
}

pub(super) unsafe extern "C" fn srv_bossbar_set_progress(
    _ctx: *mut c_void,
    id: YogStr,
    progress: f32,
) -> bool {
    unsafe { bridge!("bossbarSetProgress", [S(id), F(progress)]).unwrap_or(false) }
}

pub(super) unsafe extern "C" fn srv_bossbar_set_color(_ctx: *mut c_void, id: YogStr, color: YogStr) -> bool {
    unsafe { bridge!("bossbarSetColor", [S(id), S(color)]).unwrap_or(false) }
}

pub(super) unsafe extern "C" fn srv_bossbar_add_player(_ctx: *mut c_void, id: YogStr, player: YogStr) -> bool {
    unsafe { bridge!("bossbarAddPlayer", [S(id), S(player)]).unwrap_or(false) }
}

pub(super) unsafe extern "C" fn srv_bossbar_remove_player(
    _ctx: *mut c_void,
    id: YogStr,
    player: YogStr,
) -> bool {
    unsafe { bridge!("bossbarRemovePlayer", [S(id), S(player)]).unwrap_or(false) }
}

pub(super) unsafe extern "C" fn srv_bossbar_set_visible(_ctx: *mut c_void, id: YogStr, visible: bool) -> bool {
    unsafe { bridge!("bossbarSetVisible", [S(id), Z(visible)]).unwrap_or(false) }
}

pub(super) unsafe extern "C" fn srv_get_block_nbt(
    _ctx: *mut c_void,
    dim: YogStr,
    pos: YogBlockPos,
) -> YogOwnedStr {
    unsafe {
        owned(bridge!(
            "getBlockNbt",
            [S(dim), I(pos.x), I(pos.y), I(pos.z)]
        ))
    }
}

pub(super) unsafe extern "C" fn srv_set_block_nbt(
    _ctx: *mut c_void,
    dim: YogStr,
    pos: YogBlockPos,
    snbt: YogStr,
) -> bool {
    unsafe {
        bridge!(
            "setBlockNbt",
            [S(dim), I(pos.x), I(pos.y), I(pos.z), S(snbt)]
        )
        .unwrap_or(false)
    }
}

pub(super) unsafe extern "C" fn srv_get_inventory_slot(
    _ctx: *mut c_void,
    dim: YogStr,
    pos: yog_abi::YogBlockPos,
    slot: u32,
) -> YogOwnedStr {
    unsafe {
        owned(bridge!(
            "getInventorySlot",
            [S(dim), I(pos.x), I(pos.y), I(pos.z), I(slot as i32)]
        ))
    }
}

pub(super) unsafe extern "C" fn srv_set_inventory_slot(
    _ctx: *mut c_void,
    dim: YogStr,
    pos: yog_abi::YogBlockPos,
    slot: u32,
    item_id: YogStr,
    count: u32,
) -> bool {
    unsafe {
        bridge!(
            "setInventorySlot",
            [
                S(dim),
                I(pos.x),
                I(pos.y),
                I(pos.z),
                I(slot as i32),
                S(item_id),
                I(count as i32)
            ]
        )
        .unwrap_or(false)
    }
}

pub(super) unsafe extern "C" fn srv_player_inventory(_ctx: *mut c_void, player: YogStr) -> YogOwnedStr {
    unsafe { owned(bridge!("playerInventory", [S(player)])) }
}

pub(super) unsafe extern "C" fn srv_player_set_slot(
    _ctx: *mut c_void,
    player: YogStr,
    slot: u32,
    item_id: YogStr,
    count: u32,
) -> bool {
    unsafe {
        bridge!(
            "playerSetSlot",
            [S(player), I(slot as i32), S(item_id), I(count as i32)]
        )
        .unwrap_or(false)
    }
}

pub(super) unsafe extern "C" fn srv_player_teleport_dim(
    _ctx: *mut c_void,
    player: YogStr,
    dim: YogStr,
    pos: YogVec3,
) -> bool {
    unsafe {
        bridge!(
            "teleportToDim",
            [S(player), S(dim), D(pos.x), D(pos.y), D(pos.z)]
        )
        .unwrap_or(false)
    }
}

pub(super) unsafe extern "C" fn srv_entity_teleport_dim(
    _ctx: *mut c_void,
    uuid: YogStr,
    dim: YogStr,
    pos: YogVec3,
) -> bool {
    unsafe {
        bridge!(
            "entityTeleportToDim",
            [S(uuid), S(dim), D(pos.x), D(pos.y), D(pos.z)]
        )
        .unwrap_or(false)
    }
}

pub(super) unsafe extern "C" fn srv_online_players(_ctx: *mut c_void) -> YogOwnedStr {
    unsafe { owned(bridge!("onlinePlayers", [])) }
}

pub(super) unsafe extern "C" fn srv_world_entity_count(
    _ctx: *mut c_void,
    dim: YogStr,
    entity_type: YogStr,
) -> i32 {
    unsafe { bridge!("worldEntityCount", [S(dim), S(entity_type)]).unwrap_or(-1) }
}

pub(super) unsafe extern "C" fn srv_game_dir(_ctx: *mut c_void) -> YogOwnedStr {
    unsafe { owned(bridge!("gameDir", [])) }
}

pub(super) unsafe extern "C" fn srv_world_dir(_ctx: *mut c_void) -> YogOwnedStr {
    unsafe { owned(bridge!("worldDir", [])) }
}

pub(super) unsafe extern "C" fn srv_entity_get_nbt(_ctx: *mut c_void, uuid: YogStr) -> YogOwnedStr {
    unsafe { owned(bridge!("entityGetNbt", [S(uuid)])) }
}

pub(super) unsafe extern "C" fn srv_entity_set_nbt(_ctx: *mut c_void, uuid: YogStr, snbt: YogStr) -> bool {
    unsafe { bridge!("entitySetNbt", [S(uuid), S(snbt)]).unwrap_or(false) }
}

pub(super) unsafe extern "C" fn srv_spawn_particles(
    _ctx: *mut c_void,
    dim: YogStr,
    pos: YogVec3,
    particle_type: YogStr,
    count: i32,
    dx: f64,
    dy: f64,
    dz: f64,
    speed: f64,
) -> bool {
    unsafe {
        bridge!(
            "spawnParticles",
            [
                S(dim),
                D(pos.x),
                D(pos.y),
                D(pos.z),
                S(particle_type),
                I(count),
                D(dx),
                D(dy),
                D(dz),
                D(speed)
            ]
        )
        .unwrap_or(false)
    }
}

pub(super) unsafe extern "C" fn srv_entity_attribute_value(_ctx: *mut c_void, uuid: YogStr, attr: YogStr) -> f64 {
    unsafe { bridge!("entityAttributeValue", [S(uuid), S(attr)]).unwrap_or(f64::NAN) }
}

pub(super) unsafe extern "C" fn api_client_attribute_value(_ctx: *mut c_void, attr: YogStr) -> f64 {
    unsafe { host!("dev/yog/YogClientApi", "attributeValue", [S(attr)]).unwrap_or(f64::NAN) }
}

pub(super) unsafe extern "C" fn srv_advancement_grant(_ctx: *mut c_void, uuid: YogStr, id: YogStr) -> bool {
    unsafe { bridge!("advancementGrant", [S(uuid), S(id)]).unwrap_or(false) }
}

pub(super) unsafe extern "C" fn srv_advancement_done(_ctx: *mut c_void, uuid: YogStr, id: YogStr) -> bool {
    unsafe { bridge!("advancementDone", [S(uuid), S(id)]).unwrap_or(false) }
}

/// Whether this client's player has advancement `id` — also what books ask
/// of their locked entries (see `nativeInit`).
/// The items of item tag `id`, as this client has them.
pub(super) fn client_tag_items(id: &str) -> Vec<String> {
    let items: Option<String> = unsafe { host!("dev/yog/YogClientApi", "tagItems", [JArg::S(YogStr::from_str(id))]) };
    items.map(|s| s.split(yog_ids::sep::FIELD).filter(|i| !i.is_empty()).map(str::to_string).collect()).unwrap_or_default()
}

pub(super) fn client_advancement_done(id: &str) -> bool {
    unsafe { host!("dev/yog/YogClientApi", "advancementDone", [JArg::S(YogStr::from_str(id))]).unwrap_or(false) }
}
