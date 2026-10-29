//! The server tick's own work: startup grants, entity data sent, scheduled tasks.
use crate::game::entity_data;
use crate::jni::util::guard;
use crate::mods::handlers::handlers;
use substrate_lock::Locked;
use yog_abi::YogStr;

/// The command tag a player carries once startup grant `id` was given. It's
/// saved with the player's own data, so it follows the player — not their
/// name or uuid (a dev client gets a new name each launch, while a
/// singleplayer world's owner stays the same player).
pub(crate) fn grant_tag(id: &str) -> String {
    format!("yog.grant.{id}")
}

/// Queues `player`'s startup grants (given a few ticks after the join).
pub(crate) fn queue_startup_grants(player: &str, uuid: &str) {
    let h = handlers();
    if !h.startup_grants.is_empty() {
        h.pending_grants
            .locked()
            .push((player.to_owned(), uuid.to_owned(), 0));
    }
}

/// After the mods' tick: grants due, entity data changed this tick sent to
/// who sees it, and the scheduler's tasks due (run outside its lock — a task
/// may schedule more).
pub(crate) fn after_tick() {
    guard("startup_grants", process_pending_grants);
    guard("entity data", entity_data::flush);
}

/// Try to fulfil pending startup grants. Called each server tick; a grant is
/// only marked as given (tagged on the player) after `give_item` succeeds.
pub(crate) fn process_pending_grants() {
    let h = handlers();
    let Some(mut pending) = h.pending_grants.try_locked() else {
        return;
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
                ok &= crate::bus::call(&yog_event::player::GiveItem {
                    player: p.clone(),
                    item: item_id.to_owned(),
                    count: 1,
                })
                .unwrap_or(false);
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
