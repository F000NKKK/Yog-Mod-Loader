//! JNI: ABI minor 9 entry points.

use super::*;

// ── ABI minor 9 JNI entry points ─────────────────────────────────────────────

#[unsafe(no_mangle)]
pub extern "system" fn Java_dev_yog_NativeBridge_nativeOnItemPickupPre<'l>(
    mut env: JNIEnv<'l>,
    _class: JClass<'l>,
    player: JString<'l>,
    player_uuid: JString<'l>,
    item_id: JString<'l>,
    item_count: jint,
    entity_uuid: JString<'l>,
    dimension: JString<'l>,
) -> jni::sys::jboolean {
    let h = handlers();
    if h.item_pickup.is_empty() {
        return 1;
    }
    let p = jstr!(env, player, 1);
    let pu = jstr!(env, player_uuid, 1);
    let ii = jstr!(env, item_id, 1);
    let eu = jstr!(env, entity_uuid, 1);
    let d = jstr!(env, dimension, 1);
    let ev = YogItemPickupEvent {
        player: YogStr::from_str(&p),
        player_uuid: YogStr::from_str(&pu),
        item_id: YogStr::from_str(&ii),
        item_count: item_count as u32,
        entity_uuid: YogStr::from_str(&eu),
        dimension: YogStr::from_str(&d),
    };
    let srv = srv_ptr();
    allowed(&h.item_pickup, |ud, f| unsafe { f(ud, srv, &ev, 0) }) as jni::sys::jboolean
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_dev_yog_NativeBridge_nativeOnItemPickup<'l>(
    mut env: JNIEnv<'l>,
    _class: JClass<'l>,
    player: JString<'l>,
    player_uuid: JString<'l>,
    item_id: JString<'l>,
    item_count: jint,
    entity_uuid: JString<'l>,
    dimension: JString<'l>,
) {
    let h = handlers();
    if h.item_pickup.is_empty() {
        return;
    }
    let (p, pu) = (jstr!(env, player), jstr!(env, player_uuid));
    let (ii, eu, d) = (
        jstr!(env, item_id),
        jstr!(env, entity_uuid),
        jstr!(env, dimension),
    );
    let ev = YogItemPickupEvent {
        player: YogStr::from_str(&p),
        player_uuid: YogStr::from_str(&pu),
        item_id: YogStr::from_str(&ii),
        item_count: item_count as u32,
        entity_uuid: YogStr::from_str(&eu),
        dimension: YogStr::from_str(&d),
    };
    let srv = srv_ptr();
    guard("on_item_pickup", || {
        each(&h.item_pickup, |ud, f| unsafe { f(ud, srv, &ev, 1) });
    });
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_dev_yog_NativeBridge_nativeOnPlayerMove<'l>(
    mut env: JNIEnv<'l>,
    _class: JClass<'l>,
    player: JString<'l>,
    player_uuid: JString<'l>,
    x: jdouble,
    y: jdouble,
    z: jdouble,
    yaw: jfloat,
    pitch: jfloat,
    dimension: JString<'l>,
) {
    let h = handlers();
    if h.player_move.is_empty() {
        return;
    }
    let (p, pu, d) = (
        jstr!(env, player),
        jstr!(env, player_uuid),
        jstr!(env, dimension),
    );
    let ev = YogPlayerMoveEvent {
        player: YogStr::from_str(&p),
        player_uuid: YogStr::from_str(&pu),
        x,
        y,
        z,
        yaw,
        pitch,
        dimension: YogStr::from_str(&d),
    };
    let srv = srv_ptr();
    guard("on_player_move", || {
        each(&h.player_move, |ud, f| unsafe { f(ud, srv, &ev, 1) });
    });
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_dev_yog_NativeBridge_nativeOnContainerOpenPre<'l>(
    mut env: JNIEnv<'l>,
    _class: JClass<'l>,
    player: JString<'l>,
    player_uuid: JString<'l>,
    dimension: JString<'l>,
) -> jni::sys::jboolean {
    let h = handlers();
    if h.container_open.is_empty() {
        return 1;
    }
    let p = jstr!(env, player, 1);
    let pu = jstr!(env, player_uuid, 1);
    let d = jstr!(env, dimension, 1);
    let ev = YogContainerOpenEvent {
        player: YogStr::from_str(&p),
        player_uuid: YogStr::from_str(&pu),
        container_type: YogStr::EMPTY,
        dimension: YogStr::from_str(&d),
    };
    let srv = srv_ptr();
    allowed(&h.container_open, |ud, f| unsafe { f(ud, srv, &ev, 0) }) as jni::sys::jboolean
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_dev_yog_NativeBridge_nativeOnContainerOpen<'l>(
    mut env: JNIEnv<'l>,
    _class: JClass<'l>,
    player: JString<'l>,
    player_uuid: JString<'l>,
    container_type: JString<'l>,
    dimension: JString<'l>,
) {
    let h = handlers();
    if h.container_open.is_empty() {
        return;
    }
    let (p, pu, ct, d) = (
        jstr!(env, player),
        jstr!(env, player_uuid),
        jstr!(env, container_type),
        jstr!(env, dimension),
    );
    let ev = YogContainerOpenEvent {
        player: YogStr::from_str(&p),
        player_uuid: YogStr::from_str(&pu),
        container_type: YogStr::from_str(&ct),
        dimension: YogStr::from_str(&d),
    };
    let srv = srv_ptr();
    guard("on_container_open", || {
        each(&h.container_open, |ud, f| unsafe { f(ud, srv, &ev, 1) });
    });
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_dev_yog_NativeBridge_nativeOnContainerClose<'l>(
    mut env: JNIEnv<'l>,
    _class: JClass<'l>,
    player: JString<'l>,
    player_uuid: JString<'l>,
    dimension: JString<'l>,
) {
    let h = handlers();
    if h.container_close.is_empty() {
        return;
    }
    let (p, pu, d) = (
        jstr!(env, player),
        jstr!(env, player_uuid),
        jstr!(env, dimension),
    );
    let ev = YogContainerCloseEvent {
        player: YogStr::from_str(&p),
        player_uuid: YogStr::from_str(&pu),
        dimension: YogStr::from_str(&d),
    };
    let srv = srv_ptr();
    guard("on_container_close", || {
        each(&h.container_close, |ud, f| unsafe { f(ud, srv, &ev, 1) });
    });
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_dev_yog_NativeBridge_nativeOnProjectileHitPre<'l>(
    mut env: JNIEnv<'l>,
    _class: JClass<'l>,
    projectile_type: JString<'l>,
    projectile_uuid: JString<'l>,
    shooter_uuid: JString<'l>,
    hit_type: JString<'l>,
    hit_entity_uuid: JString<'l>,
    x: jdouble,
    y: jdouble,
    z: jdouble,
    dimension: JString<'l>,
) -> jni::sys::jboolean {
    let h = handlers();
    if h.projectile_hit.is_empty() {
        return 1;
    }
    let pt = jstr!(env, projectile_type, 1);
    let pu = jstr!(env, projectile_uuid, 1);
    let su = jstr!(env, shooter_uuid, 1);
    let ht = jstr!(env, hit_type, 1);
    let heu = jstr!(env, hit_entity_uuid, 1);
    let dim = jstr!(env, dimension, 1);
    let ev = YogProjectileHitEvent {
        projectile_type: YogStr::from_str(&pt),
        projectile_uuid: YogStr::from_str(&pu),
        shooter_uuid: YogStr::from_str(&su),
        hit_type: YogStr::from_str(&ht),
        hit_entity_uuid: YogStr::from_str(&heu),
        x,
        y,
        z,
        dimension: YogStr::from_str(&dim),
    };
    let srv = srv_ptr();
    allowed(&h.projectile_hit, |ud, f| unsafe { f(ud, srv, &ev, 0) }) as jni::sys::jboolean
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_dev_yog_NativeBridge_nativeOnProjectileHit<'l>(
    mut env: JNIEnv<'l>,
    _class: JClass<'l>,
    projectile_type: JString<'l>,
    projectile_uuid: JString<'l>,
    shooter_uuid: JString<'l>,
    hit_type: JString<'l>,
    hit_entity_uuid: JString<'l>,
    x: jdouble,
    y: jdouble,
    z: jdouble,
    dimension: JString<'l>,
) {
    let h = handlers();
    if h.projectile_hit.is_empty() {
        return;
    }
    let (pt, pu) = (jstr!(env, projectile_type), jstr!(env, projectile_uuid));
    let (su, ht) = (jstr!(env, shooter_uuid), jstr!(env, hit_type));
    let (heu, dim) = (jstr!(env, hit_entity_uuid), jstr!(env, dimension));
    let ev = YogProjectileHitEvent {
        projectile_type: YogStr::from_str(&pt),
        projectile_uuid: YogStr::from_str(&pu),
        shooter_uuid: YogStr::from_str(&su),
        hit_type: YogStr::from_str(&ht),
        hit_entity_uuid: YogStr::from_str(&heu),
        x,
        y,
        z,
        dimension: YogStr::from_str(&dim),
    };
    let srv = srv_ptr();
    guard("on_projectile_hit", || {
        each(&h.projectile_hit, |ud, f| unsafe { f(ud, srv, &ev, 1) });
    });
}
