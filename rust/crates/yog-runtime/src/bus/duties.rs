//! The runtime's own work on the bus: it listens like a mod does, before
//! the mods (cleanup for a new world) or after them (what their handlers
//! queued this tick), and provides what only it knows (the declared commands).

use std::os::raw::c_void;
use std::sync::Mutex;

use yog_abi::{YogBytes, YogReply, YogStatus, status};
use yog_event::{Event, Message, Service, client, command, keyed, player, server};

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

/// Answers with the `fn(&S) -> S::Reply` in `ud`.
unsafe extern "C" fn serve<S: Service>(ud: *mut c_void, payload: YogBytes, reply: *const YogReply) -> YogStatus {
    let f: fn(&S) -> S::Reply = unsafe { std::mem::transmute::<*mut c_void, fn(&S) -> S::Reply>(ud) };
    let Some(request) = S::from_bytes(unsafe { payload.as_slice() }) else {
        return status::BAD_PAYLOAD;
    };
    let mut answer = None;
    crate::guard(std::any::type_name::<S>(), || answer = Some(f(&request)));
    match answer {
        Some(a) => {
            unsafe { YogReply::send(reply, &a.to_bytes()) };
            status::OK
        }
        None => status::FAILED,
    }
}

/// `f` answers every `S`.
fn provide<S: Service>(f: fn(&S) -> S::Reply) {
    let at = ROUTER.resolve(S::ADDRESS);
    ROUTER.provide(at, Handler::Native { owner: None, ud: f as usize, f: serve::<S> });
}

/// The commands mods declared, for the host to register.
static DECLARED: Mutex<Vec<command::Declare>> = Mutex::new(Vec::new());

/// `f` on every `E` after the game did it, at `priority`.
fn after<E: Event>(priority: i32, f: fn(&E)) {
    listen(E::ADDRESS, priority, f);
}

/// `f` on every `E` at `address`, at `priority`.
fn listen<E: Event>(address: &str, priority: i32, f: fn(&E)) {
    let at = ROUTER.resolve(address);
    ROUTER.on(at, priority, Handler::Native { owner: None, ud: f as usize, f: run::<E> });
}

/// The runtime's subscriptions; once, at start.
pub(crate) fn register() {
    // Grants pending for a previous world's players are void.
    after::<server::life::Started>(FIRST, |_| crate::handlers().pending_grants.locked().clear());
    // Startup grants: giving items inside the join fails for the host of a
    // fresh world (not fully in it yet), so they're given a few ticks later.
    after::<player::session::Join>(LAST, |e| crate::ticks::queue_startup_grants(&e.player_name, &e.uuid));
    after::<server::game::Tick>(LAST, |_| crate::ticks::after_tick());
    after::<server::life::Stopping>(LAST, |_| crate::entity_data::server_stopped());
    // A hot reload declares a mod's commands again: each is kept once.
    provide::<command::Declare>(|d| {
        let mut all = DECLARED.locked();
        if !all.contains(d) {
            all.push(d.clone());
        }
        true
    });
    provide::<command::Declared>(|_| DECLARED.locked().clone());
    // What the server tells this client about entities comes on the runtime's own channel.
    listen::<client::net::Packet>(&keyed(client::net::Packet::ADDRESS, crate::entity_data::ENTITY_DATA), FIRST, |p| {
        crate::entity_data::receive(&p.payload)
    });
}
