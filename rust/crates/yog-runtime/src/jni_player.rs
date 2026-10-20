//! JNI: block, chat, player, item and entity events.

use super::*;

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
