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
    /// Stable server action table — pass to handlers.
    pub server: *const YogServer,

    // ── content ──────────────────────────────────────────────────────────────
    /// Register content of `kind` (`item`, `block`, `inventory`, `recipe`,
    /// `book`, `dimension`, `startup_grant`) named `id`: its definition as
    /// a Yog Data blob (`yog_data::binary`, `len` bytes at `data`), the
    /// shape of `yog_registry`'s / `yog_inventory`'s definitions, a recipe
    /// in Yog's format, a book, a dimension. One call for every kind of
    /// content — new kinds need no new ABI.
    pub register_content: unsafe extern "C" fn(ctx: *mut c_void, kind: YogStr, id: YogStr, data: *const u8, len: usize),


    // ── ABI minor 10 — client-side events ────────────────────────────────────
    pub on_hud_render: unsafe extern "C" fn(ctx: *mut c_void, ud: *mut c_void, h: YogHudRenderFn),

    // ── ABI minor 14 — world render ──────────────────────────────────────────
    /// Register a handler that fires after world geometry is rendered.
    /// `gfx.view_proj` and `gfx.camera_pos` are filled; use them to project
    /// custom 3D geometry into clip space.
    pub on_world_render:
        unsafe extern "C" fn(ctx: *mut c_void, ud: *mut c_void, h: YogWorldRenderFn),

    // ── ABI minor 19 — UI system ──────────────────────────────────────────────
    /// Register a UI tree. `ui_id` is the unique identifier (e.g. "mymod:menu").
    /// `layout_json` is the serialized layout tree.
    /// `handler` is called when an interactive element is clicked/keyed.
    pub register_ui: unsafe extern "C" fn(
        ctx: *mut c_void,
        ui_id: YogStr,
        layout_json: YogStr,
        ud: *mut c_void,
        h: YogUIEventFn,
    ),

    // ── ABI minor 20 — per-UI screen renderer ────────────────────────────────
    /// Register a render callback that fires during `YogUIScreen.render()` for a
    /// specific UI id — i.e. AFTER the screen darkening, unlike `on_hud_render`.
    /// Reuses `YogHudRenderFn` (`fn(ud, gfx)`); `gfx.screen_w/h` are set.
    /// Clicks are forwarded as `"click:X:Y"` events to the `register_ui` handler
    /// so mods can do their own hit-testing with their stored layout.
    pub on_ui_render:
        unsafe extern "C" fn(ctx: *mut c_void, ui_id: YogStr, ud: *mut c_void, h: YogHudRenderFn),

    // ── ABI minor 22 — menu entry registration ──────────────────────────────
    /// Register a button/link that the host renders on vanilla screens
    /// (TitleScreen on Fabric, ModListScreen on Forge/NeoForge).
    /// `label` is the human-readable button text (e.g. "Yog Mods").
    /// `ui_id` is the Yog UI to open when clicked (e.g. "example:menu").
    pub register_menu_entry: unsafe extern "C" fn(ctx: *mut c_void, label: YogStr, ui_id: YogStr),

    // ── ABI minor 23 — installed mods listing ───────────────────────────────
    /// All installed mods as TSV, one per line:
    /// `source \t id \t name \t version \t authors \t description`
    /// where `source` is `yog` (a .yog mod) or `platform` (a loader mod, e.g.
    /// a Fabric/Forge jar). Tabs and newlines inside fields become spaces.
    /// Callable at any time after registration (also client-side).
    pub mods_list: unsafe extern "C" fn(ctx: *mut c_void) -> YogOwnedStr,
    /// Free a `YogOwnedStr` returned by an api-table call (same allocator as
    /// `YogServer::free_str`).
    pub free_str: unsafe extern "C" fn(ptr: *mut u8, len: u32),

    // ── ABI minor 24 — open a Yog UI screen ─────────────────────────────────
    /// Open the Yog UI registered as `ui_id` on the client (schedules onto the
    /// render thread). Client-side only; a no-op on dedicated servers.
    /// `modal` blocks game input; `pause` pauses a singleplayer game.
    pub ui_open: unsafe extern "C" fn(ctx: *mut c_void, ui_id: YogStr, modal: bool, pause: bool),

    // ── ABI minor 26 — inter-mod communication ─────────────────────────────
    /// Export a function pointer under `symbol` for the calling mod (`mod_id`).
    /// Other mods can import it via `interop_import`.
    pub interop_export:
        unsafe extern "C" fn(ctx: *mut c_void, mod_id: YogStr, symbol: YogStr, ptr: *const c_void),
    /// Import a function pointer exported by `mod_id` under `symbol`.
    /// Returns null if the symbol is not (yet) registered.
    pub interop_import:
        unsafe extern "C" fn(ctx: *mut c_void, mod_id: YogStr, symbol: YogStr) -> *const c_void,

    // ── ABI minor 29 — custom dimensions ────────────────────────────────────
    /// Register a chunk generator for a previously-declared dimension `id`.
    /// `handler` is called once per chunk column that needs generating —
    /// see `YogChunkWriterApi`/`YogChunkGeneratorFn`.
    pub register_chunk_generator:
        unsafe extern "C" fn(ctx: *mut c_void, id: YogStr, ud: *mut c_void, h: YogChunkGeneratorFn),

    // ── ABI minor 33 — closing Yog UI screens, consuming UI key events ──────
    /// Close the Yog UI screen showing `ui_id` (empty = whichever Yog UI
    /// screen is open). Scheduled onto the render thread, so it's safe to
    /// call from that screen's own event handler. Client-side only.
    pub ui_close: unsafe extern "C" fn(ctx: *mut c_void, ui_id: YogStr),
    /// Called from inside a `register_ui` handler while it handles a `key:`
    /// or `char:` event: the mod used the key, so the host skips its default
    /// handling (Esc closing the screen, Tab focus navigation, ...).
    pub ui_consume_event: unsafe extern "C" fn(ctx: *mut c_void),

    // ── ABI minor 33 — translations ─────────────────────────────────────────
    /// The text of translation key `key` (e.g. `mymod.screen.title`) in the
    /// game's current language, from the loaded lang files — resource packs
    /// and `.yog` mods' `assets/<ns>/lang/*.json` (compiled from `.yres`).
    /// `YogOwnedStr::NONE` when no language defines the key. Format
    /// specifiers (`%s`, `%1$s`) come back as-is. Free with `free_str`.
    /// Client-side; a dedicated server only knows `en_us`.
    pub translate: unsafe extern "C" fn(ctx: *mut c_void, key: YogStr) -> YogOwnedStr,

    // ── ABI minor 1 — client: screen flags, sounds, held items, keys ────────
    /// Like `ui_open`, with [`ui_flags`] bits: `MODAL`, `PAUSE`,
    /// `NO_BACKGROUND` (no dimmed/blurred world behind the screen).
    /// Client-side only.
    pub ui_open_flags: unsafe extern "C" fn(ctx: *mut c_void, ui_id: YogStr, flags: u32),
    /// Play sound event `sound_id` for the local player only: at the
    /// player's position, or (`looping`) as a loop that follows the player
    /// until `client_stop_sound`. Returns a handle (0 = failed).
    pub client_play_sound: unsafe extern "C" fn(
        ctx: *mut c_void,
        sound_id: YogStr,
        volume: f32,
        pitch: f32,
        looping: bool,
    ) -> u32,
    /// Stop a sound started by `client_play_sound` (0 or unknown: no-op).
    pub client_stop_sound: unsafe extern "C" fn(ctx: *mut c_void, handle: u32),
    /// Item id in the local player's main hand (`off_hand` false) or off
    /// hand, `YogOwnedStr::NONE` for air or no player. Free with `free_str`.
    pub client_held_item: unsafe extern "C" fn(ctx: *mut c_void, off_hand: bool) -> YogOwnedStr,
    /// Tints the block at `pos` in `dim` on this client (its faces with a
    /// tint index), `color` 0xRRGGBB; -1 clears it.
    pub client_set_block_tint: unsafe extern "C" fn(ctx: *mut c_void, dim: YogStr, pos: YogBlockPos, color: i64),
    /// Whether GLFW key `key` is held down right now (client window).
    pub client_key_down: unsafe extern "C" fn(ctx: *mut c_void, key: i32) -> bool,

    // ── ABI 2.1 — book pages drawn by mods ──────────────────────────────────
    /// Draw the pictures of book pages of type `drawn` whose `drawer` is
    /// `drawer` (`hexcasting:pattern`) — see [`YogPageDrawFn`].
    pub on_book_page:
        unsafe extern "C" fn(ctx: *mut c_void, drawer: YogStr, ud: *mut c_void, f: YogPageDrawFn),

    // ── ABI 2.2 — pictures in item tooltips ─────────────────────────────────
    /// A picture under the tooltip of item `item` (Hex Casting's scrolls
    /// show their pattern): `size` says whether a stack gets one and how
    /// big, `draw` draws it — see [`YogTooltipSizeFn`], [`YogTooltipDrawFn`].
    pub on_item_tooltip: unsafe extern "C" fn(
        ctx: *mut c_void,
        item: YogStr,
        ud: *mut c_void,
        size: YogTooltipSizeFn,
        draw: YogTooltipDrawFn,
    ),

    // ── ABI 2.3 — other mods ────────────────────────────────────────────────
    /// Whether mod `id` is loaded — a Yog mod or a Java one (Fabric's,
    /// Forge's or NeoForge's mod list).
    pub is_mod_loaded: unsafe extern "C" fn(ctx: *mut c_void, id: YogStr) -> bool,
    /// Calls into a Java mod's API by reflection: `expr` is a chain from a
    /// class — `virtuoel.pehkui.api.ScaleTypes.BASE.getScaleData($0).getScale()`
    /// — `$n` the `n`-th of `args`, a list in Yog Data text (an entity is
    /// `@entity:<uuid>`). The result in Yog Data text, or
    /// [`YogOwnedStr::NONE`] when the call failed (said in the log).
    /// Free with `free_str`.
    pub java_call: unsafe extern "C" fn(ctx: *mut c_void, expr: YogStr, args: YogStr) -> YogOwnedStr,

    // ── ABI 3.0 — bodies, camera, movement (yog-body) ───────────────────────
    /// Asks the mods about entity `uuid`'s body each frame (`on`) or not.
    pub client_body_track: unsafe extern "C" fn(ctx: *mut c_void, uuid: YogStr, on: bool),
    /// The same for every entity of type `entity_type`.
    pub client_body_track_type: unsafe extern "C" fn(ctx: *mut c_void, entity_type: YogStr, on: bool),

    // ── ABI 3.1 — entity dimensions (client) ────────────────────────────────
    /// This client's side of `entity_set_dimensions` (the local player
    /// collides with it); `width` < 0 gives vanilla's back.
    pub client_set_dimensions: unsafe extern "C" fn(ctx: *mut c_void, uuid: YogStr, width: f32, height: f32, eye: f32),

    // ── ABI 3.2 — entity data (client) ──────────────────────────────────────
    /// What this client was told the entity keeps under `key`, written to
    /// `sink`; `false` if nothing.
    pub client_entity_data:
        unsafe extern "C" fn(ctx: *mut c_void, uuid: YogStr, key: YogStr, sink: *mut c_void, write: YogBytesSink) -> bool,

    // ── ABI 3.6 — attribute values (client) ─────────────────────────────────
    /// An attribute's value on this client's player (a casting grid's size
    /// from an effect, say); `f64::NAN` without a player or attribute.
    pub client_attribute_value: unsafe extern "C" fn(ctx: *mut c_void, attribute_id: YogStr) -> f64,

    // ── ABI 3.7 — advancements (client) ─────────────────────────────────────
    /// Whether this client's player has advancement `id` (what a book's
    /// locked entries wait for).
    pub client_advancement_done: unsafe extern "C" fn(ctx: *mut c_void, id: YogStr) -> bool,

    // ── ABI 3.8 — logging ───────────────────────────────────────────────────
    /// Writes `message` to the game's log at `level` (`yog_logging::Level`
    /// as a number: 0 info, 1 warn, 2 error) — where a mod's records go, so
    /// they're in the log with the game's instead of on a stdout nobody reads.
    pub log: unsafe extern "C" fn(level: u32, message: YogStr),

    // ── ABI 3.9 — the local player and what it sees (client) ────────────────
    /// Tells or does `op` (`yog_ids::client_op`) on this client with `args`
    /// (JSON): the player's state, the blocks and entities around it, its
    /// actions; the answer as JSON, or none (no player, a server).
    pub client_op: unsafe extern "C" fn(ctx: *mut c_void, op: YogStr, args: YogStr) -> YogOwnedStr,

    // ── ABI 3.10 — key bindings (client) ────────────────────────────────────
    /// A key binding the player sees and rebinds in the game's controls,
    /// under the mod's category: `id` (`mymod:xray`) names it, `name` is
    /// the translation key of its label, `default_key` a GLFW key code (-1:
    /// unbound). Declared while the mod registers; the game keeps the key.
    pub register_key_binding: unsafe extern "C" fn(ctx: *mut c_void, id: YogStr, name: YogStr, default_key: i32),

    // ── ABI 3.11 — the bus ──────────────────────────────────────────────────
    /// The bus: calls, events and services by address (`docs/design/bus.md`).
    /// Valid for the runtime's lifetime.
    pub bus: *const YogBus,
}

/// Flag bits for [`YogApi::ui_open_flags`].
pub mod ui_flags {
    /// Blocks game input (what `ui_open`'s `modal` does).
    pub const MODAL: u32 = 1;
    /// Pauses a singleplayer game.
    pub const PAUSE: u32 = 1 << 1;
    /// Draws no background behind the screen: the world stays visible.
    pub const NO_BACKGROUND: u32 = 1 << 2;
}

unsafe impl Send for YogApi {}
unsafe impl Sync for YogApi {}
