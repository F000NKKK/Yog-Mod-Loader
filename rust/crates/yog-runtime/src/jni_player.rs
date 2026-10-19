//! JNI: block, chat, player, item and entity events.

use super::*;

#[unsafe(no_mangle)]
pub extern "system" fn Java_dev_yog_NativeBridge_nativeOnBlockBreak<'l>(
    mut env: JNIEnv<'l>,
    _class: JClass<'l>,
    player: JString<'l>,
    block: JString<'l>,
    x: jint,
    y: jint,
    z: jint,
    dimension: JString<'l>,
) {
    let (p, b, d) = (jstr!(env, player), jstr!(env, block), jstr!(env, dimension));
    let ev = yog_abi::YogBlockBreakEvent {
        player: YogStr::from_str(&p),
        block: YogStr::from_str(&b),
        pos: YogBlockPos { x, y, z },
        dimension: YogStr::from_str(&d),
    };
    let srv = srv_ptr();
    guard("on_block_break", || {
        each(&handlers().block_break, |ud, f| unsafe {
            f(ud, srv, &ev, 1)
        });
    });
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_dev_yog_NativeBridge_nativeOnChat<'l>(
    mut env: JNIEnv<'l>,
    _class: JClass<'l>,
    player: JString<'l>,
    message: JString<'l>,
    dimension: JString<'l>,
) {
    let (p, m, d) = (
        jstr!(env, player),
        jstr!(env, message),
        jstr!(env, dimension),
    );
    let ev = yog_abi::YogChatEvent {
        player: YogStr::from_str(&p),
        message: YogStr::from_str(&m),
        dimension: YogStr::from_str(&d),
    };
    let srv = srv_ptr();
    guard("on_chat", || {
        each(&handlers().chat, |ud, f| unsafe { f(ud, srv, &ev, 1) });
    });
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_dev_yog_NativeBridge_nativeOnPlayerJoin<'l>(
    mut env: JNIEnv<'l>,
    _class: JClass<'l>,
    player: JString<'l>,
    uuid: JString<'l>,
    dimension: JString<'l>,
) {
    let (p, u, d) = (jstr!(env, player), jstr!(env, uuid), jstr!(env, dimension));
    let ev = yog_abi::YogPlayerEvent {
        player: YogStr::from_str(&p),
        uuid: YogStr::from_str(&u),
        dimension: YogStr::from_str(&d),
    };
    let srv = srv_ptr();
    guard("on_player_join", || {
        each(&handlers().player_join, |ud, f| unsafe {
            f(ud, srv, &ev, 1)
        });
    });
    // Queue startup grants: giving items directly inside the join callback
    // fails for the host of a freshly created world (player not fully in the
    // world yet), so grants are processed a few server ticks later.
    let h = handlers();
    if !h.startup_grants.is_empty() {
        h.pending_grants
            .locked()
            .push((p, u, 0));
    }
}

/// The command tag a player carries once startup grant `id` was given. It's
/// saved with the player's own data, so it follows the player — not their
/// name or uuid (a dev client gets a new name each launch, while a
/// singleplayer world's owner stays the same player).
pub(super) fn grant_tag(id: &str) -> String {
    format!("yog.grant.{id}")
}

/// Try to fulfil pending startup grants. Called each server tick; a grant is
/// only marked as given (tagged on the player) after `give_item` succeeds.
pub(super) fn process_pending_grants() {
    let h = handlers();
    let mut pending = match h.pending_grants.try_lock() {
        Ok(g) => g,
        Err(_) => return,
    };
    if pending.is_empty() {
        return;
    }
    // Wait a couple of ticks after join before the first attempt.
    const FIRST_TRY_TICK: u32 = 2;
    // Give up after 30 seconds — the player probably left.
    const MAX_TICKS: u32 = 600;

    let mut still_pending = Vec::new();
    for (p, u, ticks) in pending.drain(..) {
        if ticks < FIRST_TRY_TICK {
            still_pending.push((p, u, ticks + 1));
            continue;
        }
        let player = YogStr::from_str(&p);
        let mut all_ok = true;
        for sg in &h.startup_grants {
            let tag = grant_tag(&sg.id);
            let tag = YogStr::from_str(&tag);
            if unsafe { bridge!("playerHasTag", [S(player), S(tag)]) }.unwrap_or(false) {
                continue;
            }
            let book = sg.book.as_ref().map(|_| yog_ids::game::WRITTEN_BOOK);
            let mut ok = true;
            for item_id in sg.items.iter().map(String::as_str).chain(book) {
                ok &= unsafe { srv_give_item(std::ptr::null_mut(), player, YogStr::from_str(item_id), 1) };
            }
            if ok && unsafe { bridge!("addPlayerTag", [S(player), S(tag)]) }.unwrap_or(false) {
                yog_logging::info!("startup grant {} given to {}", sg.id, p);
            } else {
                all_ok = false;
            }
        }
        if !all_ok && ticks < MAX_TICKS {
            still_pending.push((p, u, ticks + 1));
        }
    }
    *pending = still_pending;
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_dev_yog_NativeBridge_nativeOnPlayerLeave<'l>(
    mut env: JNIEnv<'l>,
    _class: JClass<'l>,
    player: JString<'l>,
    uuid: JString<'l>,
    dimension: JString<'l>,
) {
    let (p, u, d) = (jstr!(env, player), jstr!(env, uuid), jstr!(env, dimension));
    let ev = yog_abi::YogPlayerEvent {
        player: YogStr::from_str(&p),
        uuid: YogStr::from_str(&u),
        dimension: YogStr::from_str(&d),
    };
    let srv = srv_ptr();
    guard("on_player_leave", || {
        each(&handlers().player_leave, |ud, f| unsafe {
            f(ud, srv, &ev, 1)
        });
    });
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_dev_yog_NativeBridge_nativeOnUseItem<'l>(
    mut env: JNIEnv<'l>,
    _class: JClass<'l>,
    player: JString<'l>,
    item: JString<'l>,
    sneaking: jni::sys::jboolean,
    dimension: JString<'l>,
) {
    let (p, i, d) = (jstr!(env, player), jstr!(env, item), jstr!(env, dimension));
    let ev = yog_abi::YogUseItemEvent {
        player: YogStr::from_str(&p),
        item: YogStr::from_str(&i),
        sneaking: sneaking != 0,
        dimension: YogStr::from_str(&d),
    };
    let srv = srv_ptr();
    guard("on_use_item", || {
        each(&handlers().use_item, |ud, f| unsafe { f(ud, srv, &ev, 1) });
    });
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_dev_yog_NativeBridge_nativeOnUseItemPre<'l>(
    mut env: JNIEnv<'l>,
    _class: JClass<'l>,
    player: JString<'l>,
    item: JString<'l>,
    sneaking: jni::sys::jboolean,
    dimension: JString<'l>,
) -> jni::sys::jboolean {
    let h = handlers();
    if h.use_item.is_empty() {
        return 1;
    }
    let p = jstr!(env, player, 1);
    let i = jstr!(env, item, 1);
    let d = jstr!(env, dimension, 1);
    let ev = yog_abi::YogUseItemEvent {
        player: YogStr::from_str(&p),
        item: YogStr::from_str(&i),
        sneaking: sneaking != 0,
        dimension: YogStr::from_str(&d),
    };
    let srv = srv_ptr();
    allowed(&h.use_item, |ud, f| unsafe { f(ud, srv, &ev, 0) }) as jni::sys::jboolean
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_dev_yog_NativeBridge_nativeOnUseBlock<'l>(
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
    let (p, b, d) = (jstr!(env, player), jstr!(env, block), jstr!(env, dimension));
    let ev = yog_abi::YogUseBlockEvent {
        player: YogStr::from_str(&p),
        block: YogStr::from_str(&b),
        pos: YogBlockPos { x, y, z },
        dimension: YogStr::from_str(&d),
        sneaking: sneaking != 0,
    };
    let srv = srv_ptr();
    guard("on_use_block", || {
        each(&handlers().use_block, |ud, f| unsafe { f(ud, srv, &ev, 1) });
    });
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_dev_yog_NativeBridge_nativeOnUseBlockPre<'l>(
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
    if h.use_block.is_empty() {
        return 1;
    }
    let p = jstr!(env, player, 1);
    let b = jstr!(env, block, 1);
    let d = jstr!(env, dimension, 1);
    let ev = yog_abi::YogUseBlockEvent {
        player: YogStr::from_str(&p),
        block: YogStr::from_str(&b),
        pos: YogBlockPos { x, y, z },
        dimension: YogStr::from_str(&d),
        sneaking: sneaking != 0,
    };
    let srv = srv_ptr();
    allowed(&h.use_block, |ud, f| unsafe { f(ud, srv, &ev, 0) }) as jni::sys::jboolean
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_dev_yog_NativeBridge_nativeOnAttackEntity<'l>(
    mut env: JNIEnv<'l>,
    _class: JClass<'l>,
    player: JString<'l>,
    target_type: JString<'l>,
    target_uuid: JString<'l>,
    dimension: JString<'l>,
) {
    let (p, tt, tu, d) = (
        jstr!(env, player),
        jstr!(env, target_type),
        jstr!(env, target_uuid),
        jstr!(env, dimension),
    );
    let ev = yog_abi::YogAttackEntityEvent {
        player: YogStr::from_str(&p),
        target_type: YogStr::from_str(&tt),
        target_uuid: YogStr::from_str(&tu),
        dimension: YogStr::from_str(&d),
    };
    let srv = srv_ptr();
    guard("on_attack_entity", || {
        each(&handlers().attack_entity, |ud, f| unsafe {
            f(ud, srv, &ev, 1)
        });
    });
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_dev_yog_NativeBridge_nativeOnEntityDamage<'l>(
    mut env: JNIEnv<'l>,
    _class: JClass<'l>,
    entity_type: JString<'l>,
    uuid: JString<'l>,
    amount: jfloat,
    source: JString<'l>,
    dimension: JString<'l>,
) {
    let (et, u, s, d) = (
        jstr!(env, entity_type),
        jstr!(env, uuid),
        jstr!(env, source),
        jstr!(env, dimension),
    );
    let ev = yog_abi::YogEntityDamageEvent {
        entity_type: YogStr::from_str(&et),
        uuid: YogStr::from_str(&u),
        amount,
        source: YogStr::from_str(&s),
        dimension: YogStr::from_str(&d),
    };
    let srv = srv_ptr();
    guard("on_entity_damage", || {
        each(&handlers().entity_damage, |ud, f| unsafe {
            f(ud, srv, &ev, 1)
        });
    });
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_dev_yog_NativeBridge_nativeOnEntityDeath<'l>(
    mut env: JNIEnv<'l>,
    _class: JClass<'l>,
    entity_type: JString<'l>,
    uuid: JString<'l>,
    source: JString<'l>,
    dimension: JString<'l>,
) {
    let (et, u, s, d) = (
        jstr!(env, entity_type),
        jstr!(env, uuid),
        jstr!(env, source),
        jstr!(env, dimension),
    );
    let ev = yog_abi::YogEntityDeathEvent {
        entity_type: YogStr::from_str(&et),
        uuid: YogStr::from_str(&u),
        source: YogStr::from_str(&s),
        dimension: YogStr::from_str(&d),
    };
    let srv = srv_ptr();
    guard("on_entity_death", || {
        each(&handlers().entity_death, |ud, f| unsafe {
            f(ud, srv, &ev, 1)
        });
    });
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_dev_yog_NativeBridge_nativeOnTick<'l>(
    _env: JNIEnv<'l>,
    _class: JClass<'l>,
) {
    let h = handlers();
    let srv = srv_ptr();
    guard("on_tick", || {
        each(&h.server_tick, |ud, f| unsafe { f(ud, srv) });
    });

    guard("startup_grants", process_pending_grants);

    // Entity data changed this tick, to who sees it — after the mods' ticks.
    guard("entity data", entity_data::flush);

    // Scheduler: what's due this tick, run outside the lock (a task may schedule more).
    let due = h.scheduler.locked().due();
    for (module, ud, f) in due {
        if module.is_retiring() {
            continue;
        }
        let Some(_call) = module.enter() else {
            continue;
        };
        guard("scheduled task", || unsafe { f(ud, srv) });
    }
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_dev_yog_NativeBridge_nativeOnServerStarted<'l>(
    _env: JNIEnv<'l>,
    _class: JClass<'l>,
    _world_dir: JString<'l>,
) {
    // Grants pending for a previous world's players are void.
    handlers().pending_grants.locked().clear();
    let srv = srv_ptr();
    guard("on_server_started", || {
        each(&handlers().server_started, |ud, f| unsafe { f(ud, srv) });
    });
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_dev_yog_NativeBridge_nativeOnServerStopping<'l>(
    _env: JNIEnv<'l>,
    _class: JClass<'l>,
) {
    let srv = srv_ptr();
    guard("on_server_stopping", || {
        each(&handlers().server_stopping, |ud, f| unsafe { f(ud, srv) });
    });
    entity_data::server_stopped();
}
