//! The runtime's hot paths, reachable from its benchmarks (`benches/`).
//! Not an API.

use std::ffi::c_void;

use yog_abi::{YogBytes, YogReply, YogScheduledFn, YogServer, YogStatus, status};
use yog_hot_reload::{GenerationAllocator, LoadedModule};

use super::bus::{Handler, Router};
use super::{OnceTask, RepeatingTask, SchedulerState};

unsafe extern "C" fn yes(_: *mut c_void, _: YogBytes, _: *const YogReply) -> YogStatus {
    status::OK
}

fn no_java(_: i32, _: &[u8]) -> (YogStatus, Vec<u8>) {
    (status::NO_HANDLER, Vec::new())
}

unsafe extern "C" fn task(_: *mut c_void, _: *const YogServer) {}

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
    move || router.emit(event, &payload, no_java).0
}

/// A scheduler with `n` repeating tasks (every 1–20 ticks) and `n` once
/// tasks: each call is one tick's worth of finding what's due.
pub fn scheduler(n: usize) -> impl FnMut() -> usize {
    let module = LoadedModule::this_process(GenerationAllocator::new().next());
    let mut s = SchedulerState::default();
    for i in 0..n {
        let period = (i % 20) as u64 + 1;
        s.repeating_tasks.push(RepeatingTask { module: module.clone(), period, ticks_left: period, ud: std::ptr::null_mut(), f: task as YogScheduledFn });
        s.once_tasks.push(OnceTask { module: module.clone(), delay_remaining: 1_000_000, ud: std::ptr::null_mut(), f: task as YogScheduledFn });
    }
    move || s.due().len()
}
