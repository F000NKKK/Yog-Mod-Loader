//! World queries and effects (ABI minor 1).

use super::*;

// ── World queries & effects (ABI minor 1) ────────────────────────────────────
//
// Each of these forwards to a `NativeBridge` static of the same shape; the JNI
// signature is derived from the argument list so the two cannot drift apart.

pub(super) fn vec_args(v: YogVec3) -> [JArg<'static>; 3] {
    [JArg::D(v.x), JArg::D(v.y), JArg::D(v.z)]
}

pub(super) fn pos_args(p: YogBlockPos) -> [JArg<'static>; 3] {
    [JArg::I(p.x), JArg::I(p.y), JArg::I(p.z)]
}

pub(super) unsafe extern "C" fn srv_entity_info(_ctx: *mut c_void, uuid: YogStr) -> YogOwnedStr {
    unsafe { owned(bridge!("entityInfo", [JArg::S(uuid)])) }
}

pub(super) unsafe extern "C" fn srv_entities_in_box(
    _ctx: *mut c_void,
    dim: YogStr,
    min: YogVec3,
    max: YogVec3,
) -> YogOwnedStr {
    unsafe {
        let [a, b, c] = vec_args(min);
        let [d, e, f] = vec_args(max);
        owned(bridge!("entitiesInBox", [JArg::S(dim), a, b, c, d, e, f]))
    }
}

pub(super) unsafe extern "C" fn srv_raycast_block(
    _ctx: *mut c_void,
    dim: YogStr,
    from: YogVec3,
    to: YogVec3,
) -> YogOwnedStr {
    unsafe {
        let [a, b, c] = vec_args(from);
        let [d, e, f] = vec_args(to);
        owned(bridge!("raycastBlock", [JArg::S(dim), a, b, c, d, e, f]))
    }
}

pub(super) unsafe extern "C" fn srv_raycast_entity(
    _ctx: *mut c_void,
    dim: YogStr,
    from: YogVec3,
    to: YogVec3,
    exclude: YogStr,
) -> YogOwnedStr {
    unsafe {
        let [a, b, c] = vec_args(from);
        let [d, e, f] = vec_args(to);
        owned(bridge!(
            "raycastEntity",
            [JArg::S(dim), a, b, c, d, e, f, JArg::S(exclude)]
        ))
    }
}

pub(super) unsafe extern "C" fn srv_block_info(
    _ctx: *mut c_void,
    dim: YogStr,
    pos: YogBlockPos,
) -> YogOwnedStr {
    unsafe {
        let [x, y, z] = pos_args(pos);
        owned(bridge!("blockInfo", [JArg::S(dim), x, y, z]))
    }
}

pub(super) unsafe extern "C" fn srv_break_block(
    _ctx: *mut c_void,
    dim: YogStr,
    pos: YogBlockPos,
    drop: bool,
    breaker: YogStr,
) -> bool {
    unsafe {
        let [x, y, z] = pos_args(pos);
        bridge!(
            "breakBlock",
            [JArg::S(dim), x, y, z, JArg::Z(drop), JArg::S(breaker)]
        )
        .unwrap_or(false)
    }
}

pub(super) unsafe extern "C" fn srv_place_block_from_slot(
    _ctx: *mut c_void,
    player: YogStr,
    slot: u32,
    dim: YogStr,
    pos: YogBlockPos,
) -> bool {
    unsafe {
        let [x, y, z] = pos_args(pos);
        bridge!(
            "placeBlockFromSlot",
            [JArg::S(player), JArg::I(slot as i32), JArg::S(dim), x, y, z]
        )
        .unwrap_or(false)
    }
}

pub(super) unsafe extern "C" fn srv_bonemeal(_ctx: *mut c_void, dim: YogStr, pos: YogBlockPos) -> bool {
    unsafe {
        let [x, y, z] = pos_args(pos);
        bridge!("bonemeal", [JArg::S(dim), x, y, z]).unwrap_or(false)
    }
}

pub(super) unsafe extern "C" fn srv_explode(
    _ctx: *mut c_void,
    dim: YogStr,
    pos: YogVec3,
    power: f32,
    fire: bool,
    source: YogStr,
) -> bool {
    unsafe {
        let [x, y, z] = vec_args(pos);
        bridge!(
            "explode",
            [
                JArg::S(dim),
                x,
                y,
                z,
                JArg::F(power),
                JArg::Z(fire),
                JArg::S(source),
            ]
        )
        .unwrap_or(false)
    }
}

pub(super) unsafe extern "C" fn srv_tellraw(_ctx: *mut c_void, player: YogStr, json: YogStr) -> bool {
    unsafe { bridge!("tellraw", [JArg::S(player), JArg::S(json)]).unwrap_or(false) }
}

pub(super) unsafe extern "C" fn srv_entity_damage(
    _ctx: *mut c_void,
    uuid: YogStr,
    amount: f32,
    damage_type: YogStr,
    attacker: YogStr,
) -> bool {
    unsafe {
        bridge!(
            "entityDamage",
            [
                JArg::S(uuid),
                JArg::F(amount),
                JArg::S(damage_type),
                JArg::S(attacker),
            ]
        )
        .unwrap_or(false)
    }
}

pub(super) unsafe extern "C" fn srv_entity_set_fire_ticks(
    _ctx: *mut c_void,
    uuid: YogStr,
    ticks: i32,
) -> bool {
    unsafe { bridge!("entitySetFireTicks", [JArg::S(uuid), JArg::I(ticks)]).unwrap_or(false) }
}

pub(super) unsafe extern "C" fn srv_player_add_xp(_ctx: *mut c_void, player: YogStr, amount: i32) -> bool {
    unsafe { bridge!("playerAddXp", [JArg::S(player), JArg::I(amount)]).unwrap_or(false) }
}

pub(super) unsafe extern "C" fn srv_player_set_may_fly(
    _ctx: *mut c_void,
    player: YogStr,
    may_fly: bool,
) -> bool {
    unsafe { bridge!("playerSetMayFly", [JArg::S(player), JArg::Z(may_fly)]).unwrap_or(false) }
}

pub(super) unsafe extern "C" fn srv_player_info(_ctx: *mut c_void, player: YogStr) -> YogOwnedStr {
    unsafe { owned(bridge!("playerInfo", [JArg::S(player)])) }
}

pub(super) unsafe extern "C" fn srv_spawn_item(
    _ctx: *mut c_void,
    dim: YogStr,
    pos: YogVec3,
    vel: YogVec3,
    item_id: YogStr,
    count: u32,
    snbt: YogStr,
) -> YogOwnedStr {
    unsafe {
        let [a, b, c] = vec_args(pos);
        let [d, e, f] = vec_args(vel);
        owned(bridge!(
            "spawnItem",
            [
                JArg::S(dim),
                a,
                b,
                c,
                d,
                e,
                f,
                JArg::S(item_id),
                JArg::I(count as i32),
                JArg::S(snbt),
            ]
        ))
    }
}

pub(super) unsafe extern "C" fn srv_item_entity_stack(_ctx: *mut c_void, uuid: YogStr) -> YogOwnedStr {
    unsafe { owned(bridge!("itemEntityStack", [JArg::S(uuid)])) }
}

pub(super) unsafe extern "C" fn srv_actionbar_json(_ctx: *mut c_void, player: YogStr, json: YogStr) -> bool {
    unsafe { bridge!("actionbarJson", [JArg::S(player), JArg::S(json)]).unwrap_or(false) }
}

pub(super) unsafe extern "C" fn srv_use_item_on_block(
    _ctx: *mut c_void,
    dim: YogStr,
    pos: YogBlockPos,
    face: u8,
    item_id: YogStr,
    player: YogStr,
) -> bool {
    unsafe {
        let [x, y, z] = pos_args(pos);
        bridge!(
            "useItemOnBlock",
            [
                JArg::S(dim),
                x,
                y,
                z,
                JArg::I(face as i32),
                JArg::S(item_id),
                JArg::S(player),
            ]
        )
        .unwrap_or(false)
    }
}

pub(super) unsafe extern "C" fn srv_pour_fluid(
    _ctx: *mut c_void,
    dim: YogStr,
    pos: YogBlockPos,
    bucket_item_id: YogStr,
) -> bool {
    unsafe {
        let [x, y, z] = pos_args(pos);
        bridge!(
            "pourFluid",
            [JArg::S(dim), x, y, z, JArg::S(bucket_item_id)]
        )
        .unwrap_or(false)
    }
}

pub(super) unsafe extern "C" fn srv_has_entity_tag(
    _ctx: *mut c_void,
    entity_type: YogStr,
    tag: YogStr,
) -> bool {
    unsafe { bridge!("hasEntityTag", [JArg::S(entity_type), JArg::S(tag)]).unwrap_or(false) }
}

pub(super) unsafe extern "C" fn srv_item_block(_ctx: *mut c_void, item_id: YogStr) -> YogOwnedStr {
    unsafe { owned(bridge!("itemBlock", [JArg::S(item_id)])) }
}

pub(super) unsafe extern "C" fn srv_place_feature(
    _ctx: *mut c_void,
    dim: YogStr,
    pos: YogBlockPos,
    feature_id: YogStr,
) -> bool {
    unsafe {
        let [x, y, z] = pos_args(pos);
        bridge!("placeFeature", [JArg::S(dim), x, y, z, JArg::S(feature_id)]).unwrap_or(false)
    }
}

pub(super) unsafe extern "C" fn srv_item_entity_set_stack(
    _ctx: *mut c_void,
    uuid: YogStr,
    item_id: YogStr,
    count: u32,
    snbt: YogStr,
) -> bool {
    unsafe {
        bridge!(
            "itemEntitySetStack",
            [
                JArg::S(uuid),
                JArg::S(item_id),
                JArg::I(count as i32),
                JArg::S(snbt),
            ]
        )
        .unwrap_or(false)
    }
}
