//! The table passed to `yog_mod_register`.
use super::{YogBus, YogGfxApi};

/// Passed to `yog_mod_register`: the bus — everything a mod does goes
/// through it (`docs/design/bus.md`) — and the one thing that can't afford
/// a message, the frame's graphics.
///
/// When mods compiled against ABI `N` load on a runtime with ABI `M > N`:
/// fields beyond `size` are not present in the mod's view — check `size` before
/// accessing fields added in later minor versions.
#[repr(C)]
pub struct YogApi {
    pub abi_version: u32,
    /// `sizeof(YogApi)` at the runtime's build time.
    pub size: u32,
    /// The bus: calls, events and services by address (`docs/design/bus.md`).
    /// Valid for the runtime's lifetime.
    pub bus: *const YogBus,
    /// The frame's graphics while a render event (`yog:event/client/render/…`)
    /// runs on this thread; null otherwise. Valid until the event's handlers return.
    pub gfx: unsafe extern "C" fn() -> *const YogGfxApi,
}

unsafe impl Send for YogApi {}
unsafe impl Sync for YogApi {}
