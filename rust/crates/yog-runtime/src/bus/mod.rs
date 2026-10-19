//! The bus (`docs/design/bus.md`): one channel for calls, events and
//! services between Yog mods, the runtime, the Java host and Java mods.

mod java;
mod router;
#[cfg(test)]
mod tests;

use std::os::raw::c_void;
use std::sync::{LazyLock, OnceLock};

use yog_abi::{ABI_VERSION, YogAddress, YogBus, YogBytes, YogHandlerFn, YogReply, YogStatus, YogStr, status};

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
                resolve,
                call,
                emit,
                on,
                provide,
                provided,
            })
        })
        .0
}

/// Writes `bytes` to `reply` (if there's one and anything to write).
fn answer(reply: *const YogReply, bytes: &[u8]) {
    if reply.is_null() || bytes.is_empty() {
        return;
    }
    unsafe {
        let r = &*reply;
        (r.write)(r.sink, bytes.as_ptr(), bytes.len());
    }
}

/// The mod registering right now, if any — what its handlers are dropped with.
fn owner() -> Option<yog_hot_reload::LoadedModule> {
    use yog_sync::Locked;
    crate::CURRENT_REGISTRATION.locked().clone()
}

unsafe extern "C" fn resolve(_ctx: *mut c_void, address: YogStr) -> YogAddress {
    ROUTER.resolve(unsafe { address.as_str() })
}

unsafe extern "C" fn call(_ctx: *mut c_void, to: YogAddress, payload: YogBytes, reply: *const YogReply) -> YogStatus {
    let (st, out) = ROUTER.call(to, unsafe { payload.as_slice() }, java::run);
    answer(reply, &out);
    st
}

unsafe extern "C" fn emit(_ctx: *mut c_void, event: YogAddress, payload: YogBytes, reply: *const YogReply) -> YogStatus {
    let (st, out) = ROUTER.emit(event, unsafe { payload.as_slice() }, java::run);
    answer(reply, &out);
    st
}

unsafe extern "C" fn on(_ctx: *mut c_void, event: YogAddress, priority: i32, ud: *mut c_void, handler: YogHandlerFn) {
    ROUTER.on(event, priority, Handler::Native { owner: owner(), ud: ud as usize, f: handler });
}

unsafe extern "C" fn provide(_ctx: *mut c_void, address: YogAddress, ud: *mut c_void, handler: YogHandlerFn) -> YogStatus {
    let added = ROUTER.provide(address, Handler::Native { owner: owner(), ud: ud as usize, f: handler });
    if added { status::OK } else { status::FAILED }
}

unsafe extern "C" fn provided(_ctx: *mut c_void, address: YogAddress) -> bool {
    ROUTER.provided(address)
}
