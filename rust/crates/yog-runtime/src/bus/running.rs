//! Whose code runs on this thread: the mod a call comes from, so what it
//! asks for (a mirror's declaration) goes when the mod does.

use std::cell::Cell;

use yog_hot_reload::ModuleGeneration;

thread_local! {
    static RUNNING: Cell<Option<ModuleGeneration>> = const { Cell::new(None) };
}

/// A mod's code entered; the one before it is back when this drops.
pub(crate) struct Running(Option<ModuleGeneration>);

pub(crate) fn enter(generation: ModuleGeneration) -> Running {
    Running(RUNNING.replace(Some(generation)))
}

impl Drop for Running {
    fn drop(&mut self) {
        RUNNING.set(self.0);
    }
}

/// The mod calling now: whose handler runs, else whose registration.
pub(crate) fn caller() -> Option<ModuleGeneration> {
    RUNNING
        .get()
        .or_else(|| super::owner().map(|m| m.generation()))
}
