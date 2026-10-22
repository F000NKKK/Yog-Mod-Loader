//! The frame's graphics while a render event runs: mods' draw handlers
//! reach it through `YogApi::gfx`.

use std::cell::Cell;

use yog_abi::YogGfxApi;

thread_local! {
    static FRAME: Cell<*const YogGfxApi> = const { Cell::new(std::ptr::null()) };
}

/// Runs `f` with `gfx` as the frame's graphics.
pub(crate) fn drawing<R>(gfx: &YogGfxApi, f: impl FnOnce() -> R) -> R {
    let before = FRAME.with(|frame| frame.replace(gfx));
    let out = f();
    FRAME.with(|frame| frame.set(before));
    out
}

/// `YogApi::gfx`.
pub(crate) unsafe extern "C" fn api_gfx() -> *const YogGfxApi {
    crate::jni_util::guarded("api_gfx", std::ptr::null(), || FRAME.with(Cell::get))
}

/// Physical pixels per GUI pixel in the frame being drawn (1 outside one).
pub(crate) fn scale_factor() -> f32 {
    let frame = FRAME.with(Cell::get);
    // SAFETY: a non-null frame is alive while its event's handlers run.
    unsafe { frame.as_ref() }
        .map_or(1.0, |g| g.scale_factor)
        .max(1.0)
}
