//! The handlers' function-pointer types, and the client frames they see.

use super::*;

// ── Handler function-pointer types ────────────────────────────────────────────
//
// All event handlers receive a `phase: u8` argument:
//   0 = Pre  — fires before the action; return value matters (false = cancel).
//   1 = Post — fires after the action; return value is ignored.
//
// This unified signature lets one registered closure handle both phases.
//
// Client-side handlers (minor 10) do NOT receive a `YogServer*` — they run on
// the render thread and have no server context.

pub type YogItemInsertFn =
    unsafe extern "C" fn(*mut c_void, *const YogServer, *const YogItemInsertEvent, u8) -> bool;

/// Packet events — always Post, no phase.
pub type YogPacketFn = unsafe extern "C" fn(*mut c_void, *const YogServer, *const YogPacketEvent);
/// Command handler.
pub type YogCommandFn = unsafe extern "C" fn(
    ud: *mut c_void,
    srv: *const YogServer,
    ev: *const YogCommandEvent,
    reply_buf: *mut u8,
    reply_cap: u32,
    reply_len: *mut u32,
);
/// Scheduler handler (once or repeating).
pub type YogScheduledFn = unsafe extern "C" fn(*mut c_void, *const YogServer);

// ── ABI minor 10 — client-side function pointer types ────────────────────────

/// UI event for a `register_ui` screen. `event_id` is one of
/// `click:X:Y`, `drag:X:Y`, `release:X:Y`, `scroll:DY` and (ABI minor 33)
/// `key:KEY:MODS` — a GLFW key code pressed or repeating, with GLFW modifier
/// bits — and `char:CODEPOINT` — a typed character (Unicode scalar value).
/// Call `YogApi::ui_consume_event` while handling `key:`/`char:` to stop the
/// host's default handling of that key (Esc closing the screen, ...).
pub type YogUIEventFn = unsafe extern "C" fn(ud: *mut c_void, ui_id: YogStr, event_id: YogStr);
/// HUD render — `gfx` is the graphics context for this frame; only valid for
/// the call duration.  `draw2d_*` functions in `gfx` work here.
pub type YogHudRenderFn = unsafe extern "C" fn(ud: *mut c_void, gfx: *const YogGfxApi);
/// A book page's picture: `data`/`len` is the page's `data` (Yog Data,
/// binary), `x, y, w, h` its box in screen pixels; `gfx` is valid for the
/// call.
pub type YogPageDrawFn = unsafe extern "C" fn(
    ud: *mut c_void,
    gfx: *const YogGfxApi,
    data: *const u8,
    len: usize,
    x: f32,
    y: f32,
    w: f32,
    h: f32,
);
/// Whether a stack of `item` with custom data `snbt` gets a tooltip
/// picture; if so its size in GUI pixels goes to `w`, `h`.
pub type YogTooltipSizeFn =
    unsafe extern "C" fn(ud: *mut c_void, item: YogStr, snbt: YogStr, w: *mut f32, h: *mut f32) -> bool;
/// Draw the tooltip picture of a stack of `item` (custom data `snbt`) in
/// the box `x, y, w, h` (screen pixels); `gfx` is valid for the call.
pub type YogTooltipDrawFn = unsafe extern "C" fn(
    ud: *mut c_void,
    gfx: *const YogGfxApi,
    item: YogStr,
    snbt: YogStr,
    x: f32,
    y: f32,
    w: f32,
    h: f32,
);
/// Where a callback writes text it gives back: `write(sink, text)`, the
/// text copied by the caller during the call.
pub type YogTextSink = unsafe extern "C" fn(sink: *mut c_void, text: YogStr);
/// The name and tooltip lines of a stack of `item` with custom data
/// `snbt`: `false` for the item's own; else one `write` of the name (empty:
/// the item's own) and the lines that stand instead of its description,
/// separated by `\n`.
/// Takes bytes a handler writes (copies them; valid for the call).
pub type YogBytesSink = unsafe extern "C" fn(sink: *mut c_void, data: *const u8, len: usize);
/// A body frame (`yog_body::Frame`, its wire form): the handler writes its
/// ops (`yog_body::Ops`) to `sink`.
pub type YogBodyFn = unsafe extern "C" fn(ud: *mut c_void, frame: *const u8, len: usize, sink: *mut c_void, write: YogBytesSink);
/// The camera this frame; the handler changes `out` (each handler sees the
/// last one's result).
pub type YogCameraFn = unsafe extern "C" fn(ud: *mut c_void, frame: *const YogCameraFrame, out: *mut YogCameraOut);
/// The local player's travel step: `true` and `velocity` set replaces it.
pub type YogTravelFn = unsafe extern "C" fn(ud: *mut c_void, frame: *const YogTravelFrame, velocity: *mut [f32; 3]) -> bool;

/// See `yog_body::CameraFrame`.
#[repr(C)]
#[derive(Copy, Clone)]
pub struct YogCameraFrame {
    pub uuid: YogStr,
    pub partial: f32,
    pub pos: [f64; 3],
    pub yaw: f32,
    pub pitch: f32,
    pub detached: bool,
    pub flags: u32,
}

/// See `yog_body::CameraOut`.
#[repr(C)]
#[derive(Copy, Clone)]
pub struct YogCameraOut {
    pub local_offset: [f32; 3],
    pub world_offset: [f32; 3],
    pub yaw: f32,
    pub pitch: f32,
    pub roll: f32,
    pub fov: f32,
}

/// See `yog_body::TravelFrame`.
#[repr(C)]
#[derive(Copy, Clone)]
pub struct YogTravelFrame {
    pub uuid: YogStr,
    pub velocity: [f32; 3],
    pub look: [f32; 3],
    pub yaw: f32,
    pub pitch: f32,
    pub forward: f32,
    pub strafe: f32,
    pub jumping: bool,
    pub sneaking: bool,
    pub gravity: f32,
    pub flags: u32,
}

pub type YogItemTextFn =
    unsafe extern "C" fn(ud: *mut c_void, item: YogStr, snbt: YogStr, sink: *mut c_void, write: YogTextSink) -> bool;
/// World render — `gfx` contains `view_proj` and `camera_pos` for 3D rendering.
/// Valid only for the call duration.
pub type YogWorldRenderFn = unsafe extern "C" fn(ud: *mut c_void, gfx: *const YogGfxApi);
