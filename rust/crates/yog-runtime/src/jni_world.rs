//! JNI: spawns, placing, deaths, advancements, interactions, crafting, explosions.

use super::*;

#[unsafe(no_mangle)]
pub extern "system" fn Java_dev_yog_NativeBridge_nativeOnEntitySpawn<'l>(
    mut env: JNIEnv<'l>,
    _class: JClass<'l>,
    entity_type: JString<'l>,
    uuid: JString<'l>,
    dimension: JString<'l>,
) {
    let h = handlers();
    if h.entity_spawn.is_empty() {
        return;
    }
    let (et, u, d) = (
        jstr!(env, entity_type),
        jstr!(env, uuid),
        jstr!(env, dimension),
    );
    let ev = yog_abi::YogEntitySpawnEvent {
        entity_type: YogStr::from_str(&et),
        uuid: YogStr::from_str(&u),
        dimension: YogStr::from_str(&d),
    };
    let srv = srv_ptr();
    guard("on_entity_spawn", || {
        each(&h.entity_spawn, |ud, f| unsafe { f(ud, srv, &ev, 1) });
    });
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_dev_yog_NativeBridge_nativeOnEntitySpawnPre<'l>(
    mut env: JNIEnv<'l>,
    _class: JClass<'l>,
    entity_type: JString<'l>,
    uuid: JString<'l>,
    dimension: JString<'l>,
) -> jni::sys::jboolean {
    let h = handlers();
    if h.entity_spawn.is_empty() {
        return 1;
    }
    let et = jstr!(env, entity_type, 1);
    let u = jstr!(env, uuid, 1);
    let d = jstr!(env, dimension, 1);
    let ev = yog_abi::YogEntitySpawnEvent {
        entity_type: YogStr::from_str(&et),
        uuid: YogStr::from_str(&u),
        dimension: YogStr::from_str(&d),
    };
    let srv = srv_ptr();
    allowed(&h.entity_spawn, |ud, f| unsafe { f(ud, srv, &ev, 0) }) as jni::sys::jboolean
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_dev_yog_NativeBridge_nativeOnEntityDamagePre<'l>(
    mut env: JNIEnv<'l>,
    _class: JClass<'l>,
    entity_type: JString<'l>,
    uuid: JString<'l>,
    amount: jfloat,
    source: JString<'l>,
    dimension: JString<'l>,
) -> jni::sys::jboolean {
    let h = handlers();
    if h.entity_damage.is_empty() {
        return 1;
    }
    let et = jstr!(env, entity_type, 1);
    let u = jstr!(env, uuid, 1);
    let s = jstr!(env, source, 1);
    let d = jstr!(env, dimension, 1);
    let ev = yog_abi::YogEntityDamageEvent {
        entity_type: YogStr::from_str(&et),
        uuid: YogStr::from_str(&u),
        amount,
        source: YogStr::from_str(&s),
        dimension: YogStr::from_str(&d),
    };
    let srv = srv_ptr();
    allowed(&h.entity_damage, |ud, f| unsafe { f(ud, srv, &ev, 0) }) as jni::sys::jboolean
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_dev_yog_NativeBridge_nativeOnPlaceBlockPre<'l>(
    mut env: JNIEnv<'l>,
    _class: JClass<'l>,
    player: JString<'l>,
    block: JString<'l>,
    x: jint,
    y: jint,
    z: jint,
    dimension: JString<'l>,
    sneaking: jni::sys::jboolean,
) -> jni::sys::jboolean {
    let h = handlers();
    if h.player_place_block.is_empty() {
        return 1;
    }
    let p = jstr!(env, player, 1);
    let b = jstr!(env, block, 1);
    let d = jstr!(env, dimension, 1);
    let ev = YogPlaceBlockEvent {
        player: YogStr::from_str(&p),
        block: YogStr::from_str(&b),
        pos: YogBlockPos { x, y, z },
        dimension: YogStr::from_str(&d),
        sneaking: sneaking != 0,
    };
    let srv = srv_ptr();
    allowed(&h.player_place_block, |ud, f| unsafe { f(ud, srv, &ev, 0) }) as jni::sys::jboolean
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_dev_yog_NativeBridge_nativeOnPlaceBlock<'l>(
    mut env: JNIEnv<'l>,
    _class: JClass<'l>,
    player: JString<'l>,
    block: JString<'l>,
    x: jint,
    y: jint,
    z: jint,
    dimension: JString<'l>,
    sneaking: jni::sys::jboolean,
) {
    let h = handlers();
    if h.player_place_block.is_empty() {
        return;
    }
    let (p, b, d) = (jstr!(env, player), jstr!(env, block), jstr!(env, dimension));
    let ev = YogPlaceBlockEvent {
        player: YogStr::from_str(&p),
        block: YogStr::from_str(&b),
        pos: YogBlockPos { x, y, z },
        dimension: YogStr::from_str(&d),
        sneaking: sneaking != 0,
    };
    let srv = srv_ptr();
    guard("on_player_place_block", || {
        each(&h.player_place_block, |ud, f| unsafe { f(ud, srv, &ev, 1) });
    });
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_dev_yog_NativeBridge_nativeOnPlayerDeathPre<'l>(
    mut env: JNIEnv<'l>,
    _class: JClass<'l>,
    player: JString<'l>,
    uuid: JString<'l>,
    source: JString<'l>,
    dimension: JString<'l>,
) -> jni::sys::jboolean {
    let h = handlers();
    if h.player_death.is_empty() {
        return 1;
    }
    let p = jstr!(env, player, 1);
    let u = jstr!(env, uuid, 1);
    let s = jstr!(env, source, 1);
    let d = jstr!(env, dimension, 1);
    let ev = YogPlayerDeathEvent {
        player: YogStr::from_str(&p),
        uuid: YogStr::from_str(&u),
        source: YogStr::from_str(&s),
        dimension: YogStr::from_str(&d),
    };
    let srv = srv_ptr();
    allowed(&h.player_death, |ud, f| unsafe { f(ud, srv, &ev, 0) }) as jni::sys::jboolean
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_dev_yog_NativeBridge_nativeOnPlayerDeath<'l>(
    mut env: JNIEnv<'l>,
    _class: JClass<'l>,
    player: JString<'l>,
    uuid: JString<'l>,
    source: JString<'l>,
    dimension: JString<'l>,
) {
    let h = handlers();
    if h.player_death.is_empty() {
        return;
    }
    let (p, u, s, d) = (
        jstr!(env, player),
        jstr!(env, uuid),
        jstr!(env, source),
        jstr!(env, dimension),
    );
    let ev = YogPlayerDeathEvent {
        player: YogStr::from_str(&p),
        uuid: YogStr::from_str(&u),
        source: YogStr::from_str(&s),
        dimension: YogStr::from_str(&d),
    };
    let srv = srv_ptr();
    guard("on_player_death", || {
        each(&h.player_death, |ud, f| unsafe { f(ud, srv, &ev, 1) });
    });
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_dev_yog_NativeBridge_nativeOnPlayerRespawn<'l>(
    mut env: JNIEnv<'l>,
    _class: JClass<'l>,
    player: JString<'l>,
    uuid: JString<'l>,
    at_anchor: jni::sys::jboolean,
    dimension: JString<'l>,
) {
    let h = handlers();
    if h.player_respawn.is_empty() {
        return;
    }
    let (p, u, d) = (jstr!(env, player), jstr!(env, uuid), jstr!(env, dimension));
    let ev = YogPlayerRespawnEvent {
        player: YogStr::from_str(&p),
        uuid: YogStr::from_str(&u),
        at_anchor: at_anchor != 0,
        dimension: YogStr::from_str(&d),
    };
    let srv = srv_ptr();
    guard("on_player_respawn", || {
        each(&h.player_respawn, |ud, f| unsafe { f(ud, srv, &ev, 1) });
    });
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_dev_yog_NativeBridge_nativeOnAdvancement<'l>(
    mut env: JNIEnv<'l>,
    _class: JClass<'l>,
    player: JString<'l>,
    uuid: JString<'l>,
    advancement: JString<'l>,
    dimension: JString<'l>,
) {
    let h = handlers();
    if h.advancement.is_empty() {
        return;
    }
    let (p, u, a, d) = (
        jstr!(env, player),
        jstr!(env, uuid),
        jstr!(env, advancement),
        jstr!(env, dimension),
    );
    let ev = YogAdvancementEvent {
        player: YogStr::from_str(&p),
        uuid: YogStr::from_str(&u),
        advancement: YogStr::from_str(&a),
        dimension: YogStr::from_str(&d),
    };
    let srv = srv_ptr();
    guard("on_advancement", || {
        each(&h.advancement, |ud, f| unsafe { f(ud, srv, &ev, 1) });
    });
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_dev_yog_NativeBridge_nativeOnEntityInteractPre<'l>(
    mut env: JNIEnv<'l>,
    _class: JClass<'l>,
    player: JString<'l>,
    player_uuid: JString<'l>,
    entity_type: JString<'l>,
    entity_uuid: JString<'l>,
    hand: JString<'l>,
    dimension: JString<'l>,
) -> jni::sys::jboolean {
    let h = handlers();
    if h.entity_interact.is_empty() {
        return 1;
    }
    let p = jstr!(env, player, 1);
    let pu = jstr!(env, player_uuid, 1);
    let et = jstr!(env, entity_type, 1);
    let eu = jstr!(env, entity_uuid, 1);
    let ha = jstr!(env, hand, 1);
    let d = jstr!(env, dimension, 1);
    let ev = YogEntityInteractEvent {
        player: YogStr::from_str(&p),
        player_uuid: YogStr::from_str(&pu),
        entity_type: YogStr::from_str(&et),
        entity_uuid: YogStr::from_str(&eu),
        hand: YogStr::from_str(&ha),
        dimension: YogStr::from_str(&d),
    };
    let srv = srv_ptr();
    allowed(&h.entity_interact, |ud, f| unsafe { f(ud, srv, &ev, 0) }) as jni::sys::jboolean
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_dev_yog_NativeBridge_nativeOnEntityInteract<'l>(
    mut env: JNIEnv<'l>,
    _class: JClass<'l>,
    player: JString<'l>,
    player_uuid: JString<'l>,
    entity_type: JString<'l>,
    entity_uuid: JString<'l>,
    hand: JString<'l>,
    dimension: JString<'l>,
) {
    let h = handlers();
    if h.entity_interact.is_empty() {
        return;
    }
    let (p, pu) = (jstr!(env, player), jstr!(env, player_uuid));
    let (et, eu, ha, d) = (
        jstr!(env, entity_type),
        jstr!(env, entity_uuid),
        jstr!(env, hand),
        jstr!(env, dimension),
    );
    let ev = YogEntityInteractEvent {
        player: YogStr::from_str(&p),
        player_uuid: YogStr::from_str(&pu),
        entity_type: YogStr::from_str(&et),
        entity_uuid: YogStr::from_str(&eu),
        hand: YogStr::from_str(&ha),
        dimension: YogStr::from_str(&d),
    };
    let srv = srv_ptr();
    guard("on_entity_interact", || {
        each(&h.entity_interact, |ud, f| unsafe { f(ud, srv, &ev, 1) });
    });
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_dev_yog_NativeBridge_nativeOnItemCraft<'l>(
    mut env: JNIEnv<'l>,
    _class: JClass<'l>,
    player: JString<'l>,
    player_uuid: JString<'l>,
    result_item: JString<'l>,
    result_count: jint,
    dimension: JString<'l>,
) {
    let h = handlers();
    if h.item_craft.is_empty() {
        return;
    }
    let (p, pu, ri, d) = (
        jstr!(env, player),
        jstr!(env, player_uuid),
        jstr!(env, result_item),
        jstr!(env, dimension),
    );
    let ev = YogCraftEvent {
        player: YogStr::from_str(&p),
        player_uuid: YogStr::from_str(&pu),
        result_item: YogStr::from_str(&ri),
        result_count: result_count as u32,
        dimension: YogStr::from_str(&d),
    };
    let srv = srv_ptr();
    guard("on_item_craft", || {
        each(&h.item_craft, |ud, f| unsafe { f(ud, srv, &ev, 1) });
    });
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_dev_yog_NativeBridge_nativeOnExplosionPre<'l>(
    mut env: JNIEnv<'l>,
    _class: JClass<'l>,
    dimension: JString<'l>,
    x: jdouble,
    y: jdouble,
    z: jdouble,
    power: jfloat,
    cause_uuid: JString<'l>,
) -> jni::sys::jboolean {
    let h = handlers();
    if h.explosion.is_empty() {
        return 1;
    }
    let d = jstr!(env, dimension, 1);
    let cu = jstr!(env, cause_uuid, 1);
    let ev = YogExplosionEvent {
        dimension: YogStr::from_str(&d),
        x,
        y,
        z,
        power,
        cause_uuid: YogStr::from_str(&cu),
    };
    let srv = srv_ptr();
    allowed(&h.explosion, |ud, f| unsafe { f(ud, srv, &ev, 0) }) as jni::sys::jboolean
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_dev_yog_NativeBridge_nativeOnExplosion<'l>(
    mut env: JNIEnv<'l>,
    _class: JClass<'l>,
    dimension: JString<'l>,
    x: jdouble,
    y: jdouble,
    z: jdouble,
    power: jfloat,
    cause_uuid: JString<'l>,
) {
    let h = handlers();
    if h.explosion.is_empty() {
        return;
    }
    let (d, cu) = (jstr!(env, dimension), jstr!(env, cause_uuid));
    let ev = YogExplosionEvent {
        dimension: YogStr::from_str(&d),
        x,
        y,
        z,
        power,
        cause_uuid: YogStr::from_str(&cu),
    };
    let srv = srv_ptr();
    guard("on_explosion", || {
        each(&h.explosion, |ud, f| unsafe { f(ud, srv, &ev, 1) });
    });
}
