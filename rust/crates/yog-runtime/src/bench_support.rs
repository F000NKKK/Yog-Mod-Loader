//! The runtime's hot paths, reachable from its benchmarks (`benches/`).
//! Not an API.

use std::ffi::c_void;

use yog_abi::{YogScheduledFn, YogServer};
use yog_hot_reload::{GenerationAllocator, LoadedModule};

use super::{Entry, OnceTask, RepeatingTask, SchedulerState, allowed};

type Ping = unsafe extern "C" fn(*mut c_void) -> bool;

unsafe extern "C" fn yes(_: *mut c_void) -> bool {
    true
}

unsafe extern "C" fn task(_: *mut c_void, _: *const YogServer) {}

/// A pre-phase event over `n` live mods' handlers that all allow it:
/// each call dispatches it once.
pub fn dispatch(n: usize) -> impl FnMut() -> bool {
    let module = LoadedModule::this_process(GenerationAllocator::new().next());
    let entries: Vec<Entry<Ping>> = (0..n).map(|_| (module.clone(), std::ptr::null_mut(), yes as Ping)).collect();
    move || allowed(&entries, |ud, f| unsafe { f(ud) })
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
