//! The runtime's hot paths, reachable from its benchmarks (`benches/`).
//! Not an API.

use std::ffi::c_void;

use yog_abi::{YogBytes, YogReply, YogStatus, status};
use yog_hot_reload::{GenerationAllocator, LoadedModule};

use super::bus::{Handler, Router};

unsafe extern "C" fn yes(_: *mut c_void, _: YogBytes, _: *const YogReply) -> YogStatus {
    status::OK
}

fn no_java(_: i32, _: &[u8], _: &YogReply) -> YogStatus {
    status::NO_HANDLER
}


/// An event over `n` live mods' handlers that all let it happen: each
/// call emits it once through the bus's router.
pub fn dispatch(n: usize) -> impl FnMut() -> YogStatus {
    let module = LoadedModule::this_process(GenerationAllocator::new().next());
    let router = Router::default();
    let event = router.resolve("bench:event");
    for _ in 0..n {
        router.on(event, 0, Handler::Native { owner: Some(module.clone()), ud: 0, f: yes });
    }
    let payload = [0u8; 16];
    let reply = YogReply::discard();
    move || router.emit(event, &payload, &reply, no_java)
}


/// The bus with a real JVM: `benches/bus_calls.rs`.
pub mod jvm;
