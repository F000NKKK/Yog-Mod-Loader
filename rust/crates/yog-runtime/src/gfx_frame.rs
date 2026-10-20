//! The frame's graphics while a render event runs: mods' draw handlers
//! reach it through `YogApi::gfx`.

use std::cell::Cell;
use std::os::raw::c_void;

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
pub(crate) unsafe extern "C" fn api_gfx(_ctx: *mut c_void) -> *const YogGfxApi {
    FRAME.with(Cell::get)
}
