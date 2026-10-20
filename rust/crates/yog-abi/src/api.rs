//! The registration table passed to `yog_mod_register`.

use super::*;

// ── Registration table (passed to yog_mod_register) ──────────────────────────

/// Passed to `yog_mod_register`. Call the function pointers here to register
/// handlers, commands, content, and schedulers.
///
/// When mods compiled against ABI `N` load on a runtime with ABI `M > N`:
/// fields beyond `size` are not present in the mod's view — check `size` before
/// accessing fields added in later minor versions.
#[repr(C)]
pub struct YogApi {
    pub abi_version: u32,
    /// `sizeof(YogApi)` at the runtime's build time.
    pub size: u32,
    /// Opaque pointer to runtime handler storage.
    pub ctx: *mut c_void,

    // ── content ──────────────────────────────────────────────────────────────
    /// Register content of `kind` (`item`, `block`, `inventory`, `recipe`,
    /// `book`, `dimension`, `startup_grant`) named `id`: its definition as
    /// a Yog Data blob (`yog_data::binary`, `len` bytes at `data`), the
    /// shape of `yog_registry`'s / `yog_inventory`'s definitions, a recipe
    /// in Yog's format, a book, a dimension. One call for every kind of
    /// content — new kinds need no new ABI.
    pub register_content: unsafe extern "C" fn(ctx: *mut c_void, kind: YogStr, id: YogStr, data: *const u8, len: usize),

    /// Free a `YogOwnedStr` returned by an api-table call (same allocator as
    /// `YogApi::free_str`).
    pub free_str: unsafe extern "C" fn(ptr: *mut u8, len: u32),

    // ── ABI minor 26 — inter-mod communication ─────────────────────────────
    /// Export a function pointer under `symbol` for the calling mod (`mod_id`).
    /// Other mods can import it via `interop_import`.
    pub interop_export:
        unsafe extern "C" fn(ctx: *mut c_void, mod_id: YogStr, symbol: YogStr, ptr: *const c_void),
    /// Import a function pointer exported by `mod_id` under `symbol`.
    /// Returns null if the symbol is not (yet) registered.
    pub interop_import:
        unsafe extern "C" fn(ctx: *mut c_void, mod_id: YogStr, symbol: YogStr) -> *const c_void,

    /// Calls into a Java mod's API by reflection: `expr` is a chain from a
    /// class — `virtuoel.pehkui.api.ScaleTypes.BASE.getScaleData($0).getScale()`
    /// — `$n` the `n`-th of `args`, a list in Yog Data text (an entity is
    /// `@entity:<uuid>`). The result in Yog Data text, or
    /// [`YogOwnedStr::NONE`] when the call failed (said in the log).
    /// Free with `free_str`.
    pub java_call: unsafe extern "C" fn(ctx: *mut c_void, expr: YogStr, args: YogStr) -> YogOwnedStr,

    // ── ABI 3.8 — logging ───────────────────────────────────────────────────
    /// Writes `message` to the game's log at `level` (`yog_logging::Level`
    /// as a number: 0 info, 1 warn, 2 error) — where a mod's records go, so
    /// they're in the log with the game's instead of on a stdout nobody reads.
    pub log: unsafe extern "C" fn(level: u32, message: YogStr),

    // ── ABI 3.11 — the bus ──────────────────────────────────────────────────
    /// The bus: calls, events and services by address (`docs/design/bus.md`).
    /// Valid for the runtime's lifetime.
    pub bus: *const YogBus,
    /// The frame's graphics while a render event (`yog:event/client/render/…`)
    /// runs on this thread; null otherwise. Valid until the event's handlers return.
    pub gfx: unsafe extern "C" fn(ctx: *mut c_void) -> *const YogGfxApi,
}

unsafe impl Send for YogApi {}
unsafe impl Sync for YogApi {}
