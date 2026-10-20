//! The runtime's own work on the bus's events: it listens like a mod does,
//! before the mods (cleanup for a new world) or after them (what their
//! handlers queued this tick).

use std::os::raw::c_void;

use yog_abi::{YogBytes, YogReply, YogStatus, status};
use yog_event::{Event, PlayerJoinEvent, ServerStartedEvent, ServerStoppingEvent, ServerTickEvent};

use yog_sync::Locked;

use super::{Handler, ROUTER};

/// Before every mod's handler.
const FIRST: i32 = i32::MAX;
/// After every mod's handler.
const LAST: i32 = i32::MIN;

/// Runs the `fn(&E)` in `ud` on the payload.
unsafe extern "C" fn run<E: Event>(ud: *mut c_void, payload: YogBytes, _reply: *const YogReply) -> YogStatus {
    let f: fn(&E) = unsafe { std::mem::transmute::<*mut c_void, fn(&E)>(ud) };
    match E::from_bytes(unsafe { payload.as_slice() }) {
        Some(e) => {
            crate::guard(std::any::type_name::<E>(), || f(&e));
            status::OK
        }
        None => status::BAD_PAYLOAD,
    }
}

/// `f` on every `E` after the game did it, at `priority`.
fn after<E: Event>(priority: i32, f: fn(&E)) {
    let at = ROUTER.resolve(E::ADDRESS);
    ROUTER.on(at, priority, Handler::Native { owner: None, ud: f as usize, f: run::<E> });
}

/// The runtime's subscriptions; once, at start.
pub(crate) fn register() {
    // Grants pending for a previous world's players are void.
    after::<ServerStartedEvent>(FIRST, |_| crate::handlers().pending_grants.locked().clear());
    // Startup grants: giving items inside the join fails for the host of a
    // fresh world (not fully in it yet), so they're given a few ticks later.
    after::<PlayerJoinEvent>(LAST, |e| crate::ticks::queue_startup_grants(&e.player_name, &e.uuid));
    after::<ServerTickEvent>(LAST, |_| crate::ticks::after_tick());
    after::<ServerStoppingEvent>(LAST, |_| crate::entity_data::server_stopped());
}
