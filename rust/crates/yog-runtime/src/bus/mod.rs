//! The bus (`docs/design/bus.md`): one channel for calls, events and
//! services between Yog mods, the runtime, the Java host and Java mods.

mod arena;
#[cfg(test)]
mod arena_jvm;
mod declared;
mod describe;
mod duties;
mod java;
#[cfg(any(test, feature = "bench"))]
pub(crate) mod jvm_support;
mod own;
mod router;
#[cfg(test)]
mod tests;

use std::os::raw::c_void;
use yog_sync::{LazyLock, OnceLock};

use yog_abi::{
    ABI_VERSION, YogAddress, YogBus, YogBytes, YogHandlerFn, YogReply, YogStatus, YogStr, status,
};
use yog_event::{Event, Message, Service};

pub(crate) use describe::purge as purge_descriptors;
pub(crate) use duties::{provide as serve, register as register_duties};
pub(crate) use router::{Handler, Router};

/// The runtime's one router.
pub(crate) static ROUTER: LazyLock<Router> = LazyLock::new(Router::default);

static TABLE: OnceLock<BusTable> = OnceLock::new();

/// The table handed to mods (its pointers point at statics only).
struct BusTable(YogBus);
unsafe impl Send for BusTable {}
unsafe impl Sync for BusTable {}

/// The bus table mods get through `YogApi::bus`.
pub(crate) fn table() -> *const YogBus {
    &TABLE
        .get_or_init(|| {
            BusTable(YogBus {
                abi_version: ABI_VERSION,
                size: std::mem::size_of::<YogBus>() as u32,
                ctx: std::ptr::null_mut(),
                resolve: abi_resolve,
                call: abi_call,
                emit: abi_emit,
                on: abi_on,
                provide: abi_provide,
                provided: abi_provided,
                atom: abi_atom,
                atom_name: abi_atom_name,
            })
        })
        .0
}

/// Emits `event` before the game does it: whether every handler lets it.
pub(crate) fn allowed<E: Event>(event: &E) -> bool {
    let Some(pre) = event.pre_address() else {
        return true;
    };
    event.with_bytes(|payload| ROUTER.emit(pre, payload, &YogReply::discard(), java::run))
        != status::CANCELLED
}

/// Emits `event` after the game did it.
pub(crate) fn emitted<E: Event>(event: &E) {
    event.with_bytes(|payload| {
        ROUTER.emit(event.address(), payload, &YogReply::discard(), java::run)
    });
}

/// Emits `event` and reads back what the handlers made of it (itself when
/// the reply isn't one).
pub(crate) fn changed<E: Event>(event: E) -> E {
    let mut reply = Vec::new();
    event.with_bytes(|payload| {
        ROUTER.emit(
            event.address(),
            payload,
            &YogReply::to_vec(&mut reply),
            java::run,
        )
    });
    // Nothing written: no handler changed it.
    E::from_bytes(&reply).unwrap_or(event)
}

/// Calls `address`'s provider with `payload`: its reply, `None` when no one
/// answers — how the runtime's own `yog_event` functions reach the bus.
fn call_at(address: yog_event::Address, payload: &[u8]) -> Option<Vec<u8>> {
    let (st, reply) = ROUTER.call_vec(address, payload, java::run);
    (st == status::OK).then_some(reply)
}

/// Makes the actions' functions (`yog_event::chat::broadcast`) the runtime
/// calls reach its bus; once, at start.
pub(crate) fn install_caller() {
    yog_event::set_caller(call_at);
}

/// Calls `request`'s provider: its answer, `None` when no one answers.
pub(crate) fn call<S: Service>(request: &S) -> Option<S::Reply> {
    let (st, reply) =
        request.with_bytes(|payload| ROUTER.call_vec(request.address(), payload, java::run));
    if st != status::OK {
        return None;
    }
    S::Reply::from_bytes(&reply)
}

/// Whether anyone listens to `E` (unkeyed) — skip building a frame no one draws.
pub(crate) fn heard<E: Event>() -> bool {
    ROUTER.heard(ROUTER.resolve(E::ADDRESS))
}

/// The mod registering right now, if any — what its handlers are dropped with.
fn owner() -> Option<yog_hot_reload::LoadedModule> {
    crate::globals::CURRENT_REGISTRATION.lock().clone()
}

unsafe extern "C" fn abi_atom(_ctx: *mut c_void, name: YogStr) -> u32 {
    yog_data::atom::local::intern(unsafe { name.as_str() })
}

unsafe extern "C" fn abi_atom_name(_ctx: *mut c_void, n: u32) -> YogStr {
    yog_data::atom::local::name(n).map_or(YogStr::EMPTY, YogStr::from_str)
}

unsafe extern "C" fn abi_resolve(_ctx: *mut c_void, address: YogStr) -> YogAddress {
    ROUTER.resolve(unsafe { address.as_str() })
}

unsafe extern "C" fn abi_call(
    _ctx: *mut c_void,
    to: YogAddress,
    payload: YogBytes,
    reply: *const YogReply,
) -> YogStatus {
    // The reply goes straight to the caller's sink (none: dropped).
    let discard = YogReply::discard();
    ROUTER.call(
        to,
        unsafe { payload.as_slice() },
        unsafe { reply.as_ref() }.unwrap_or(&discard),
        java::run,
    )
}

unsafe extern "C" fn abi_emit(
    _ctx: *mut c_void,
    event: YogAddress,
    payload: YogBytes,
    reply: *const YogReply,
) -> YogStatus {
    let discard = YogReply::discard();
    ROUTER.emit(
        event,
        unsafe { payload.as_slice() },
        unsafe { reply.as_ref() }.unwrap_or(&discard),
        java::run,
    )
}

unsafe extern "C" fn abi_on(
    _ctx: *mut c_void,
    event: YogAddress,
    priority: i32,
    ud: *mut c_void,
    handler: YogHandlerFn,
) {
    ROUTER.on(
        event,
        priority,
        Handler::Native {
            owner: owner(),
            ud: ud as usize,
            f: handler,
        },
    );
}

unsafe extern "C" fn abi_provide(
    _ctx: *mut c_void,
    address: YogAddress,
    ud: *mut c_void,
    handler: YogHandlerFn,
) -> YogStatus {
    let added = ROUTER.provide(
        address,
        Handler::Native {
            owner: owner(),
            ud: ud as usize,
            f: handler,
        },
    );
    if added { status::OK } else { status::FAILED }
}

unsafe extern "C" fn abi_provided(_ctx: *mut c_void, address: YogAddress) -> bool {
    ROUTER.provided(address)
}
