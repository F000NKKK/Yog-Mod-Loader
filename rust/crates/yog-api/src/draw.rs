//! Drawing on the bus: a render event (`client/render/…`) runs its
//! handlers with the frame's graphics, reached through `YogApi::gfx`.

use yog_event::Event;
use yog_gfx::GfxContext;

use crate::bus::EventCx;
use crate::registry::{Registry, global_api};

/// The frame's graphics while a render event runs; `None` outside one.
pub(crate) fn frame_gfx() -> Option<GfxContext> {
    let api = global_api();
    if api.is_null() {
        return None;
    }
    let gfx = unsafe { ((*api).gfx)() };
    // SAFETY: the runtime keeps the frame's table alive while the event's handlers run.
    (!gfx.is_null()).then(|| unsafe { GfxContext::from_raw(gfx) })
}

impl Registry {
    /// Runs `draw` with the frame's graphics on every `E` (only `key`'s,
    /// if given — a screen's id).
    pub(crate) fn on_draw<E: Event + 'static>(
        &mut self,
        key: Option<&str>,
        draw: impl Fn(&GfxContext) + Send + Sync + 'static,
    ) {
        let handler = move |_: &E, _: &EventCx| {
            if let Some(gfx) = frame_gfx() {
                draw(&gfx);
            }
        };
        match key {
            Some(key) => self.on_key::<E, _>(key, handler),
            None => self.after::<E, _>(handler),
        }
    }
}
