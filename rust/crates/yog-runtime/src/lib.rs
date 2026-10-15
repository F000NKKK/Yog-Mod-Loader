//! Yog runtime — the native library loaded by the Fabric host.
//!
//! Exposes JNI entry points (`Java_dev_yog_NativeBridge_*`) that the host calls,
//! and a stable C ABI (`YogApi` / `YogServer`) that mods program against.
//!
//! Architecture:
//!   - `YogServer`  — a `#[repr(C)]` table of standalone JNI-calling functions
//!     that mods call to mutate the world.
//!   - `YogApi`     — a `#[repr(C)]` table of registration functions; mods call
//!     them inside `yog_mod_register` to subscribe to events.
//!   - `RuntimeHandlers` — the runtime's internal event/handler storage.
//!     Filled during `nativeInit` (write), read-only after. The scheduler sub-
//!     state uses an inner `Mutex` for safe addition during event dispatch.

use std::collections::HashMap;
use std::num::NonZeroU32;
use std::os::raw::c_void;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, OnceLock, RwLock};

use glow::HasContext;
use jcall::JArg;
use jni::objects::{JByteArray, JClass, JFloatArray, JString};
use jni::sys::{jdouble, jfloat, jint, jstring};
use jni::{JNIEnv, JavaVM};
use libloading::Symbol;
use yog_hot_reload::{HotReloader, LoadedModule, ModuleGeneration, ModuleRegistry};

use yog_abi::{
    ABI_VERSION, YogAdvancementEvent, YogAdvancementFn, YogApi, YogAttackEntityFn, YogBlockBreakFn,
    YogBlockPos, YogChatFn, YogClientFn, YogCommandFn, YogContainerCloseEvent,
    YogContainerCloseFn, YogContainerOpenEvent, YogContainerOpenFn, YogCraftEvent, YogCraftFn,
    YogEntityDamageFn, YogEntityDeathFn, YogEntityInteractEvent, YogEntityInteractFn,
    YogEntitySpawnFn, YogExplosionEvent, YogExplosionFn, YogGfxApi, YogHudRenderFn, YogItemInsertEvent, YogItemInsertFn, YogItemPickupEvent, YogItemPickupFn, YogKeyPressEvent, YogMachineCraftEvent, YogMachineCraftFn, YogKeyPressFn, YogOwnedStr, YogPacketFn,
    YogPlaceBlockEvent, YogPlaceBlockFn, YogPlayerDeathEvent, YogPlayerDeathFn, YogPlayerFn,
    YogPlayerMoveEvent, YogPlayerMoveFn, YogPlayerRespawnEvent, YogPlayerRespawnFn,
    YogProjectileHitEvent, YogProjectileHitFn, YogScheduledFn, YogScreenFn, YogServer, YogServerFn,
    YogStr, YogUseBlockFn, YogUseItemFn, YogVec3, YogWorldRenderFn,
};
use yog_registry::{BlockDef, ItemDef};

// ── Static globals ────────────────────────────────────────────────────────────

/// Cached JVM handle for any-thread callbacks.
static JAVA_VM: OnceLock<JavaVM> = OnceLock::new();
/// Loaded mod libraries, keyed by mod id — owns the safe `dlopen`/`dlclose`
/// swap lifecycle (see `yog-hot-reload`) instead of a flat keep-alive `Vec`,
/// so a reload can find "the current generation of this mod" and safely
/// retire the previous one.
static HOT_RELOADER: OnceLock<HotReloader<RuntimeModuleRegistry>> = OnceLock::new();
/// The mod currently inside its own `yog_mod_register` call. Every `api_*`
/// registration function reads this to tag what it stores with a
/// `LoadedModule` handle, instead of requiring mods to pass a mod id
/// through the stable C ABI (an ABI break, and edits to every mod's
/// source). Valid because a mod's entire registration always happens
/// synchronously inside one `yog_mod_register` call — see `load_mod_lib`
/// and `Java_dev_yog_NativeBridge_nativeHotReload`.
static CURRENT_REGISTRATION: Mutex<Option<LoadedModule>> = Mutex::new(None);
/// Metadata of loaded .yog mods: [id, name, version, authors, description,
/// name_key, description_key] (the keys empty unless the mod's yog.toml
/// gives its name/description as `.yres` texts).
static MOD_INFOS: Mutex<Vec<[String; 7]>> = Mutex::new(Vec::new());
/// Inter-mod symbol registry: mod_id → (symbol_name → function pointer).
/// Filled during `yog_mod_register` (export calls), read during import calls.
static INTEROP_SYMBOLS: OnceLock<Mutex<HashMap<String, HashMap<String, usize>>>> = OnceLock::new();
/// Stable server table (populated once in nativeInit, then read-only).
static SERVER: OnceLock<YogServer> = OnceLock::new();
/// Stable api table. Mods capture this pointer at registration (yog-api's
/// `installed_mods()` calls through it later, e.g. from UI render) — so it
/// must outlive `nativeInit`, not live on its stack. `ctx` inside points at
/// `HANDLERS`' own `RwLock`, published *before* any mod registers (unlike
/// the old design, where it pointed at a temporary pre-move `Box` that
/// went stale the instant `nativeInit` finished) — so it stays valid for a
/// hot-reload's later re-registration too.
static API_TABLE: OnceLock<YogApi> = OnceLock::new();
/// All registered handlers + content. A `RwLock`, not a plain value: unlike
/// the original one-shot-then-immutable design, a hot-reload needs to
/// append fresh entries for a reloaded mod after this has long been
/// published. Read sites (`handlers()`) take a read guard; registration
/// (initial or reload) takes a write guard — both deref straight through to
/// `RuntimeHandlers`, so existing `handlers().field` call sites are
/// unaffected by this type change.
static HANDLERS: OnceLock<RwLock<RuntimeHandlers>> = OnceLock::new();

// ── OpenGL context (client-side, render thread only) ─────────────────────────

struct GlCtx(glow::Context);
unsafe impl Send for GlCtx {}
unsafe impl Sync for GlCtx {}

/// Initialized by `nativeGlInit` on the render thread.  `None` on dedicated server.
static GL: OnceLock<GlCtx> = OnceLock::new();

// Raw GL function pointers for GL_ARB_get_program_binary (not exposed by glow 0.13).
// Captured during the glow loader callback in `nativeGlInit`.
// `None` when the extension is unavailable (very old drivers).
static GL_GET_PROGRAM_BINARY: OnceLock<Option<usize>> = OnceLock::new();
static GL_PROGRAM_BINARY: OnceLock<Option<usize>> = OnceLock::new();
static GL_GET_PROGRAM_IV: OnceLock<Option<usize>> = OnceLock::new();

// ── Handler storage ───────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct UiLayer {
    pub id: String,
    pub parent: Option<String>,
    pub modal: bool,
    pub pause_game: bool,
    pub visible: bool,
    pub enabled: bool,
    pub z_index: i32,
}

/// One command registration: owning module, typed-args schema (if any),
/// user data and callback.
/// A tooltip picture's mod, user data, size and draw callbacks.
type TooltipEntry = (LoadedModule, *mut c_void, yog_abi::YogTooltipSizeFn, yog_abi::YogTooltipDrawFn);
type ItemTextEntry = (LoadedModule, *mut c_void, yog_abi::YogItemTextFn);
type CommandEntry = (LoadedModule, Option<String>, *mut c_void, YogCommandFn);

// Every callback table below is tagged with the `LoadedModule` that
// registered it (see `current_registration()`), so a hot-reload's dispatch
// sites can skip a retiring module's stale entries (`LoadedModule::
// is_retiring()`) before they're physically swept by `RuntimeModuleRegistry
// ::purge`. Content registries — `items`/`blocks`/`sound_groups`/`inventories`/`books`/
// `recipes`/`dimensions`/`startup_grants`/`menu_entries` — are deliberately
// NOT tagged: they're one-time definitions copied into Minecraft's own
// registries at startup, not callbacks invoked later, and Minecraft itself
// has no live registry-unregister mechanism to hot-reload them against
// regardless of what this crate does.
#[derive(Default)]
struct RuntimeHandlers {
    block_break: Vec<(LoadedModule, *mut c_void, YogBlockBreakFn)>,
    chat: Vec<(LoadedModule, *mut c_void, YogChatFn)>,
    player_join: Vec<(LoadedModule, *mut c_void, YogPlayerFn)>,
    player_leave: Vec<(LoadedModule, *mut c_void, YogPlayerFn)>,
    use_item: Vec<(LoadedModule, *mut c_void, YogUseItemFn)>,
    use_block: Vec<(LoadedModule, *mut c_void, YogUseBlockFn)>,
    attack_entity: Vec<(LoadedModule, *mut c_void, YogAttackEntityFn)>,
    entity_damage: Vec<(LoadedModule, *mut c_void, YogEntityDamageFn)>,
    entity_death: Vec<(LoadedModule, *mut c_void, YogEntityDeathFn)>,
    entity_spawn: Vec<(LoadedModule, *mut c_void, YogEntitySpawnFn)>,
    player_place_block: Vec<(LoadedModule, *mut c_void, YogPlaceBlockFn)>,
    player_death: Vec<(LoadedModule, *mut c_void, YogPlayerDeathFn)>,
    player_respawn: Vec<(LoadedModule, *mut c_void, YogPlayerRespawnFn)>,
    advancement: Vec<(LoadedModule, *mut c_void, YogAdvancementFn)>,
    entity_interact: Vec<(LoadedModule, *mut c_void, YogEntityInteractFn)>,
    item_craft: Vec<(LoadedModule, *mut c_void, YogCraftFn)>,
    machine_craft: Vec<(LoadedModule, *mut c_void, YogMachineCraftFn)>,
    explosion: Vec<(LoadedModule, *mut c_void, YogExplosionFn)>,
    item_pickup: Vec<(LoadedModule, *mut c_void, YogItemPickupFn)>,
    item_insert: Vec<(LoadedModule, *mut c_void, YogItemInsertFn)>,
    player_move: Vec<(LoadedModule, *mut c_void, YogPlayerMoveFn)>,
    container_open: Vec<(LoadedModule, *mut c_void, YogContainerOpenFn)>,
    container_close: Vec<(LoadedModule, *mut c_void, YogContainerCloseFn)>,
    projectile_hit: Vec<(LoadedModule, *mut c_void, YogProjectileHitFn)>,
    client_tick: Vec<(LoadedModule, *mut c_void, YogClientFn)>,
    hud_render: Vec<(LoadedModule, *mut c_void, YogHudRenderFn)>,
    body: Vec<(LoadedModule, *mut c_void, yog_abi::YogBodyFn)>,
    camera: Vec<(LoadedModule, *mut c_void, yog_abi::YogCameraFn)>,
    travel: Vec<(LoadedModule, *mut c_void, yog_abi::YogTravelFn)>,
    world_render: Vec<(LoadedModule, *mut c_void, YogWorldRenderFn)>,
    key_press: Vec<(LoadedModule, *mut c_void, YogKeyPressFn)>,
    screen_open: Vec<(LoadedModule, *mut c_void, YogScreenFn)>,
    screen_close: Vec<(LoadedModule, *mut c_void, YogScreenFn)>,
    server_tick: Vec<(LoadedModule, *mut c_void, YogServerFn)>,
    server_started: Vec<(LoadedModule, *mut c_void, YogServerFn)>,
    server_stopping: Vec<(LoadedModule, *mut c_void, YogServerFn)>,
    /// All handlers registered under one command name, in registration order.
    /// `schema` is `None` for a plain `on_command` (bare, zero-argument)
    /// registration and `Some(schema)` for `on_typed_command`. Kept as a
    /// `Vec` (not a single slot) so a mod can register several subcommands —
    /// e.g. differing arg counts, or several bare/typed registrations sharing
    /// one top-level name — without later registrations silently clobbering
    /// earlier ones (see `nativeOnCommand`, which tries each entry whose
    /// schema's arg count matches the actual invocation, in order, until one
    /// produces a reply).
    commands: HashMap<String, Vec<CommandEntry>>,
    /// (namespace, name, the recipe in Yog's own format).
    recipes: Vec<(String, String, yog_data::Value)>,
    packets: HashMap<String, (LoadedModule, *mut c_void, YogPacketFn)>,
    client_packets: HashMap<String, (LoadedModule, *mut c_void, YogPacketFn)>,
    items: Vec<ItemDef>,
    /// Mods' entity types, as Yog Data (the host reads them whole).
    entities: Vec<yog_data::Value>,
    /// Ticks of mods' entities, by type.
    entity_ticks: HashMap<String, Vec<(LoadedModule, *mut c_void, yog_abi::YogEntityTickFn)>>,
    blocks: Vec<BlockDef>,
    /// The content blob the hosts read (`nativeContent`), built once —
    /// dropped when more content registers.
    content_blob: Mutex<Option<Arc<Vec<u8>>>>,
    /// Mods' sound groups by id; a block's `sound` is one of these or built in.
    sound_groups: HashMap<String, yog_registry::SoundGroup>,
    /// Mods' tool and armor materials by id (the game's are built in).
    tool_materials: HashMap<String, yog_registry::ToolMaterial>,
    /// Placed features added to biomes (an ore's): for the hosts that add them in code.
    biome_features: Vec<yog_registry::BiomeFeature>,
    armor_materials: HashMap<String, yog_registry::ArmorMaterial>,
    inventories: Vec<yog_inventory::InventoryDef>,
    /// The ids of the registered books (their renderers are `book_renderers`).
    books: std::collections::HashSet<String>,
    book_renderers: Mutex<HashMap<String, yog_book::BookRenderer>>,
    /// Declared dimension-type defs: id -> `YogDimensionDef` JSON. Actual
    /// `LevelStem`/`ServerLevel` creation is a host (Java) responsibility —
    /// see `yog-dimensions` crate docs.
    dimensions: HashMap<String, yog_data::Value>,
    chunk_generators: HashMap<String, (LoadedModule, *mut c_void, yog_abi::YogChunkGeneratorFn)>,
    /// Dimensions' `terrain`s (`.ydef`), generated here: by the dimension's id,
    /// which is its `generator_type`.
    terrains: HashMap<String, Arc<yog_worldgen::Generator>>,
    ui_handlers: HashMap<String, (LoadedModule, *mut c_void, yog_abi::YogUIEventFn)>,
    ui_render_handlers: HashMap<String, Vec<(LoadedModule, *mut c_void, YogHudRenderFn)>>,
    /// Book pages' pictures (`BookPage::Drawn`), by drawer id.
    page_drawers: HashMap<String, (LoadedModule, *mut c_void, yog_abi::YogPageDrawFn)>,
    /// Pictures in item tooltips, by item id: the first that answers draws.
    item_tooltips: HashMap<String, Vec<TooltipEntry>>,
    /// Stacks' own names and tooltip lines, by item.
    item_texts: HashMap<String, Vec<ItemTextEntry>>,
    menu_entries: Vec<(String, String)>, // (label, ui_id) for vanilla screen buttons
    pub active_uis: Mutex<Vec<UiLayer>>,
    startup_grants: Vec<yog_registry::StartupGrant>,
    /// Players whose startup grants are pending: (name, uuid, ticks_waited).
    /// Granting happens on a server tick after join — giving items directly in
    /// the join callback fails for the host of a freshly created world.
    pending_grants: Mutex<Vec<(String, String, u32)>>,
    scheduler: Mutex<SchedulerState>,
}

// All fn ptrs are C-ABI; ud pointers are from Box::into_raw of Send+Sync closures.
unsafe impl Send for RuntimeHandlers {}
unsafe impl Sync for RuntimeHandlers {}

#[derive(Default)]
struct SchedulerState {
    once_tasks: Vec<OnceTask>,
    repeating_tasks: Vec<RepeatingTask>,
}

impl SchedulerState {
    /// One server tick: the tasks due now — once tasks whose delay ran out
    /// (`schedule_once(d)`: `d` ticks later, 0 and 1 meaning the next tick),
    /// repeating ones whose period came round (`schedule_repeating(p)`:
    /// every `p` ticks, 0 meaning every tick). Once tasks fired are gone.
    fn due(&mut self) -> Vec<(LoadedModule, *mut c_void, YogScheduledFn)> {
        let mut out = Vec::new();
        self.once_tasks.retain_mut(|t| {
            if t.delay_remaining <= 1 {
                out.push((t.module.clone(), t.ud, t.f));
                false
            } else {
                t.delay_remaining -= 1;
                true
            }
        });
        for t in &mut self.repeating_tasks {
            t.ticks_left = t.ticks_left.saturating_sub(1);
            if t.ticks_left == 0 {
                out.push((t.module.clone(), t.ud, t.f));
                t.ticks_left = t.period.max(1);
            }
        }
        out
    }
}

struct OnceTask {
    module: LoadedModule,
    delay_remaining: u64,
    ud: *mut c_void,
    f: YogScheduledFn,
}
struct RepeatingTask {
    module: LoadedModule,
    period: u64,
    ticks_left: u64,
    ud: *mut c_void,
    f: YogScheduledFn,
}

unsafe impl Send for SchedulerState {}
unsafe impl Sync for SchedulerState {}
unsafe impl Send for OnceTask {}
unsafe impl Send for RepeatingTask {}

/// A mod callback, tagged with the module that registered it.
type Entry<F> = (LoadedModule, *mut c_void, F);

/// Calls `call(ud, f)` for every entry whose module is still live, until
/// one returns `false`; whether none did.
fn each_while<F: Copy>(entries: &[Entry<F>], mut call: impl FnMut(*mut c_void, F) -> bool) -> bool {
    for (module, ud, f) in entries {
        if module.is_retiring() {
            continue;
        }
        let Some(_call) = module.enter() else {
            continue;
        };
        if !call(*ud, *f) {
            return false;
        }
    }
    true
}

/// Calls `call(ud, f)` for every entry whose module is still live (a
/// post-phase handler's result is ignored).
fn each<F: Copy, R>(entries: &[Entry<F>], mut call: impl FnMut(*mut c_void, F) -> R) {
    each_while(entries, |ud, f| {
        call(ud, f);
        true
    });
}

/// Pre-phase dispatch: whether every live handler allows the action. The
/// first `false` cancels it and skips the rest; a panicking mod allows it.
fn allowed<F: Copy>(entries: &[Entry<F>], call: impl FnMut(*mut c_void, F) -> bool) -> bool {
    std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| each_while(entries, call)))
        .unwrap_or(true)
}

fn handlers() -> std::sync::RwLockReadGuard<'static, RuntimeHandlers> {
    HANDLERS
        .get()
        .expect("yog: nativeInit not called yet")
        .read()
        .expect("handlers lock poisoned")
}

fn current_registration() -> LoadedModule {
    CURRENT_REGISTRATION
        .lock()
        .unwrap()
        .clone()
        .expect("a yog-runtime api_* function was called outside of yog_mod_register")
}

fn hot_reloader() -> &'static HotReloader<RuntimeModuleRegistry> {
    HOT_RELOADER.get().expect("yog: nativeInit not called yet")
}

/// The `yog-hot-reload` host contract: `retarget` needs no work here because
/// `LoadedModule::is_retiring()` already makes a superseded generation's
/// dispatch-table entries inert (every `handlers().<field>` loop checks it)
/// the instant `HotReloader::reload` calls `retire` on the old module —
/// well before this is even invoked. `purge` is the actual safety-net
/// sweep, dropping every entry tagged with a retired generation right
/// before its library is hard-unloaded.
/// Runs `$body` with `$list` bound to each per-event handler list of `$h`.
macro_rules! for_each_handler_list {
    ($h:expr_2021, |$list:ident| $body:expr_2021) => {{
        {
            let $list = &mut $h.block_break;
            $body;
        }
        {
            let $list = &mut $h.chat;
            $body;
        }
        {
            let $list = &mut $h.player_join;
            $body;
        }
        {
            let $list = &mut $h.player_leave;
            $body;
        }
        {
            let $list = &mut $h.use_item;
            $body;
        }
        {
            let $list = &mut $h.use_block;
            $body;
        }
        {
            let $list = &mut $h.attack_entity;
            $body;
        }
        {
            let $list = &mut $h.entity_damage;
            $body;
        }
        {
            let $list = &mut $h.entity_death;
            $body;
        }
        {
            let $list = &mut $h.entity_spawn;
            $body;
        }
        {
            let $list = &mut $h.player_place_block;
            $body;
        }
        {
            let $list = &mut $h.player_death;
            $body;
        }
        {
            let $list = &mut $h.player_respawn;
            $body;
        }
        {
            let $list = &mut $h.advancement;
            $body;
        }
        {
            let $list = &mut $h.entity_interact;
            $body;
        }
        {
            let $list = &mut $h.item_craft;
            $body;
        }
        {
            let $list = &mut $h.machine_craft;
            $body;
        }
        {
            let $list = &mut $h.explosion;
            $body;
        }
        {
            let $list = &mut $h.item_pickup;
            $body;
        }
        {
            let $list = &mut $h.item_insert;
            $body;
        }
        {
            let $list = &mut $h.player_move;
            $body;
        }
        {
            let $list = &mut $h.container_open;
            $body;
        }
        {
            let $list = &mut $h.container_close;
            $body;
        }
        {
            let $list = &mut $h.projectile_hit;
            $body;
        }
        {
            let $list = &mut $h.client_tick;
            $body;
        }
        {
            let $list = &mut $h.hud_render;
            $body;
        }
        {
            let $list = &mut $h.body;
            $body;
        }
        {
            let $list = &mut $h.camera;
            $body;
        }
        {
            let $list = &mut $h.travel;
            $body;
        }
        {
            let $list = &mut $h.world_render;
            $body;
        }
        {
            let $list = &mut $h.key_press;
            $body;
        }
        {
            let $list = &mut $h.screen_open;
            $body;
        }
        {
            let $list = &mut $h.screen_close;
            $body;
        }
        {
            let $list = &mut $h.server_tick;
            $body;
        }
        {
            let $list = &mut $h.server_started;
            $body;
        }
        {
            let $list = &mut $h.server_stopping;
            $body;
        }
    }};
}

struct RuntimeModuleRegistry;

impl ModuleRegistry for RuntimeModuleRegistry {
    fn retarget(&self, _mod_id: &str, _from: ModuleGeneration, _to: ModuleGeneration) {}

    fn purge(&self, generation: ModuleGeneration) {
        let Some(lock) = HANDLERS.get() else { return };
        let mut h = lock.write().expect("handlers lock poisoned");
        let h = &mut *h;
        for_each_handler_list!(h, |list| list
            .retain(|(m, ..)| m.generation() != generation));
        for entries in h.commands.values_mut() {
            entries.retain(|(m, ..)| m.generation() != generation);
        }
        h.packets.retain(|_, (m, ..)| m.generation() != generation);
        h.client_packets
            .retain(|_, (m, ..)| m.generation() != generation);
        h.chunk_generators
            .retain(|_, (m, ..)| m.generation() != generation);
        h.ui_handlers
            .retain(|_, (m, ..)| m.generation() != generation);
        for entries in h.ui_render_handlers.values_mut() {
            entries.retain(|(m, ..)| m.generation() != generation);
        }
        h.page_drawers
            .retain(|_, (m, ..)| m.generation() != generation);
        for entries in h.item_texts.values_mut() {
            entries.retain(|(m, ..)| m.generation() != generation);
        }
        for entries in h.entity_ticks.values_mut() {
            entries.retain(|(m, ..)| m.generation() != generation);
        }
        for entries in h.item_tooltips.values_mut() {
            entries.retain(|(m, ..)| m.generation() != generation);
        }
        h.scheduler
            .lock()
            .expect("scheduler poisoned")
            .once_tasks
            .retain(|t| t.module.generation() != generation);
        h.scheduler
            .lock()
            .expect("scheduler poisoned")
            .repeating_tasks
            .retain(|t| t.module.generation() != generation);
    }
}

// ── JNI helpers ──────────────────────────────────────────────────────────────

fn guard(label: &str, f: impl FnOnce()) {
    if std::panic::catch_unwind(std::panic::AssertUnwindSafe(f)).is_err() {
        yog_logging::error!("a mod panicked handling `{}` (ignored)", label);
    }
}

/// Reads a Java string, returning `$or` (default: `Default::default()`)
/// from the enclosing function when it can't be read.
macro_rules! jstr {
    ($env:expr_2021, $s:expr_2021) => {
        jstr!($env, $s, Default::default())
    };
    ($env:expr_2021, $s:expr_2021, $or:expr_2021) => {
        match $env.get_string(&$s) {
            Ok(s) => String::from(s),
            Err(_) => return $or,
        }
    };
}

fn get_env() -> Option<jni::AttachGuard<'static>> {
    JAVA_VM.get()?.attach_current_thread().ok()
}

/// `<class>.<name>(args...)` on this thread's env, `Option<R>` — see
/// [`jcall!`]; `None` without a JVM too.
macro_rules! host {
    ($class:literal, $name:literal, [$($arg:expr_2021),* $(,)?]) => {
        match $crate::get_env() {
            Some(mut env) => $crate::jcall::jcall!(&mut env, $class, $name, [$($arg),*]),
            None => None,
        }
    };
}

/// `NativeBridge.<name>(args...)` — see [`host!`].
macro_rules! bridge {
    ($name:literal, [$($arg:expr_2021),* $(,)?]) => {
        host!("dev/yog/NativeBridge", $name, [$($arg),*])
    };
}

/// A host string reply handed to a mod (`None`/`null` → [`YogOwnedStr::NONE`]).
fn owned(s: Option<String>) -> YogOwnedStr {
    s.map_or(YogOwnedStr::NONE, YogOwnedStr::from_string)
}

/// The first `N` tab-separated values of a host reply.
fn parse_tsv<T: std::str::FromStr, const N: usize>(s: &str) -> Option<[T; N]> {
    let values: Vec<T> = s
        .split('\t')
        .take(N)
        .map(|v| v.parse().ok())
        .collect::<Option<_>>()?;
    values.try_into().ok()
}

// ── Free-str allocator used by YogOwnedStr ────────────────────────────────────

unsafe extern "C" fn yog_free_str(ptr: *mut u8, len: u32) {
    unsafe {
        if !ptr.is_null() {
            drop(Box::from_raw(std::ptr::slice_from_raw_parts_mut(
                ptr,
                len as usize,
            )));
        }
    }
}

// ── YogServer standalone functions (one per action) ───────────────────────────
//
// ctx is unused here — all state is in the JAVA_VM static.

unsafe extern "C" fn srv_broadcast(_ctx: *mut c_void, msg: YogStr) {
    unsafe {
        let _: Option<()> = bridge!("broadcast", [S(msg)]);
    }
}

unsafe extern "C" fn srv_get_block(
    _ctx: *mut c_void,
    dim: YogStr,
    pos: YogBlockPos,
) -> YogOwnedStr {
    unsafe { owned(bridge!("getBlock", [S(dim), I(pos.x), I(pos.y), I(pos.z)])) }
}

unsafe extern "C" fn srv_set_block(
    _ctx: *mut c_void,
    dim: YogStr,
    pos: YogBlockPos,
    block: YogStr,
) -> bool {
    unsafe {
        bridge!("setBlock", [S(dim), I(pos.x), I(pos.y), I(pos.z), S(block)]).unwrap_or(false)
    }
}

unsafe extern "C" fn srv_world_time(_ctx: *mut c_void, dim: YogStr, out: *mut i64) -> bool {
    unsafe {
        match bridge!("worldTime", [S(dim)]) {
            Some(v) if v != i64::MIN => {
                *out = v;
                true
            }
            _ => false,
        }
    }
}

unsafe extern "C" fn srv_set_time(_ctx: *mut c_void, dim: YogStr, time: i64) -> bool {
    unsafe { bridge!("worldSetTime", [S(dim), J(time)]).unwrap_or(false) }
}

unsafe extern "C" fn srv_is_raining(_ctx: *mut c_void, dim: YogStr) -> bool {
    unsafe { bridge!("worldIsRaining", [S(dim)]).unwrap_or(false) }
}

unsafe extern "C" fn srv_set_weather(
    _ctx: *mut c_void,
    dim: YogStr,
    raining: bool,
    dur: i32,
) -> bool {
    unsafe { bridge!("worldSetWeather", [S(dim), Z(raining), I(dur)]).unwrap_or(false) }
}

unsafe extern "C" fn srv_give_item(
    _ctx: *mut c_void,
    player: YogStr,
    item: YogStr,
    count: u32,
) -> bool {
    unsafe { bridge!("giveItem", [S(player), S(item), I(count as i32)]).unwrap_or(false) }
}

unsafe extern "C" fn srv_player_teleport(_ctx: *mut c_void, player: YogStr, pos: YogVec3) -> bool {
    unsafe { bridge!("teleport", [S(player), D(pos.x), D(pos.y), D(pos.z)]).unwrap_or(false) }
}

unsafe extern "C" fn srv_send_to_player(
    _ctx: *mut c_void,
    player: YogStr,
    channel: YogStr,
    data: *const u8,
    len: u32,
) -> bool {
    unsafe {
        bridge!(
            "sendToPlayer",
            [
                S(player),
                S(channel),
                Bytes(std::slice::from_raw_parts(data, len as usize))
            ]
        )
        .unwrap_or(false)
    }
}

unsafe extern "C" fn srv_send_to_server(
    _ctx: *mut c_void,
    channel: YogStr,
    data: *const u8,
    len: u32,
) -> bool {
    unsafe {
        host!(
            "dev/yog/YogClient",
            "sendToServer",
            [
                S(channel),
                Bytes(std::slice::from_raw_parts(data, len as usize))
            ]
        )
        .unwrap_or(false)
    }
}

unsafe extern "C" fn srv_kick_player(_ctx: *mut c_void, player: YogStr, reason: YogStr) -> bool {
    unsafe { bridge!("kickPlayer", [S(player), S(reason)]).unwrap_or(false) }
}

unsafe extern "C" fn srv_set_gamemode(_ctx: *mut c_void, player: YogStr, mode: YogStr) -> bool {
    unsafe { bridge!("setGamemode", [S(player), S(mode)]).unwrap_or(false) }
}

unsafe extern "C" fn srv_send_title(
    _ctx: *mut c_void,
    player: YogStr,
    title: YogStr,
    sub: YogStr,
    fi: i32,
    stay: i32,
    fo: i32,
) -> bool {
    unsafe {
        bridge!(
            "sendTitle",
            [S(player), S(title), S(sub), I(fi), I(stay), I(fo)]
        )
        .unwrap_or(false)
    }
}

unsafe extern "C" fn srv_send_actionbar(_ctx: *mut c_void, player: YogStr, msg: YogStr) -> bool {
    unsafe { bridge!("sendActionbar", [S(player), S(msg)]).unwrap_or(false) }
}

unsafe extern "C" fn srv_play_sound(
    _ctx: *mut c_void,
    dim: YogStr,
    pos: YogVec3,
    sound: YogStr,
    vol: f32,
    pitch: f32,
) -> bool {
    unsafe {
        bridge!(
            "playSound",
            [
                S(dim),
                D(pos.x),
                D(pos.y),
                D(pos.z),
                S(sound),
                F(vol),
                F(pitch)
            ]
        )
        .unwrap_or(false)
    }
}

unsafe extern "C" fn srv_play_sound_player(
    _ctx: *mut c_void,
    player: YogStr,
    sound: YogStr,
    vol: f32,
    pitch: f32,
) -> bool {
    unsafe {
        bridge!("playSoundToPlayer", [S(player), S(sound), F(vol), F(pitch)]).unwrap_or(false)
    }
}

unsafe extern "C" fn srv_entity_teleport(_ctx: *mut c_void, uuid: YogStr, pos: YogVec3) -> bool {
    unsafe { bridge!("entityTeleport", [S(uuid), D(pos.x), D(pos.y), D(pos.z)]).unwrap_or(false) }
}

unsafe extern "C" fn srv_entity_rotation(
    _ctx: *mut c_void,
    uuid: YogStr,
    out: *mut YogVec3,
) -> bool {
    unsafe {
        let Some(s): Option<String> = bridge!("entityRotation", [S(uuid)]) else {
            return false;
        };
        match parse_tsv::<f64, 2>(&s) {
            Some([yaw, pitch]) => {
                *out = YogVec3 {
                    x: yaw,
                    y: pitch,
                    z: 0.0,
                };
                true
            }
            None => false,
        }
    }
}

unsafe extern "C" fn srv_entity_position(
    _ctx: *mut c_void,
    uuid: YogStr,
    out: *mut YogVec3,
) -> bool {
    unsafe {
        let Some(s): Option<String> = bridge!("entityPosition", [S(uuid)]) else {
            return false;
        };
        match parse_tsv::<f64, 3>(&s) {
            Some([x, y, z]) => {
                *out = YogVec3 { x, y, z };
                true
            }
            None => false,
        }
    }
}

unsafe extern "C" fn srv_entity_health(_ctx: *mut c_void, uuid: YogStr, out: *mut f32) -> bool {
    unsafe {
        match bridge!("entityHealth", [S(uuid)]) {
            Some(v) if !f64::is_nan(v) => {
                *out = v as f32;
                true
            }
            _ => false,
        }
    }
}

unsafe extern "C" fn srv_entity_set_health(_ctx: *mut c_void, uuid: YogStr, hp: f32) -> bool {
    unsafe { bridge!("entitySetHealth", [S(uuid), D(hp as f64)]).unwrap_or(false) }
}

unsafe extern "C" fn srv_entity_kill(_ctx: *mut c_void, uuid: YogStr) -> bool {
    unsafe { bridge!("entityKill", [S(uuid)]).unwrap_or(false) }
}

unsafe extern "C" fn srv_spawn_entity(
    _ctx: *mut c_void,
    type_id: YogStr,
    dim: YogStr,
    pos: YogVec3,
) -> YogOwnedStr {
    unsafe {
        owned(bridge!(
            "spawnEntity",
            [S(type_id), S(dim), D(pos.x), D(pos.y), D(pos.z)]
        ))
    }
}

unsafe extern "C" fn srv_entity_add_effect(
    _ctx: *mut c_void,
    uuid: YogStr,
    fx: YogStr,
    dur: i32,
    amp: u8,
    particles: bool,
) -> bool {
    unsafe {
        bridge!(
            "entityAddEffect",
            [S(uuid), S(fx), I(dur), I(amp as i32), Z(particles)]
        )
        .unwrap_or(false)
    }
}

unsafe extern "C" fn srv_entity_remove_effect(_ctx: *mut c_void, uuid: YogStr, fx: YogStr) -> bool {
    unsafe { bridge!("entityRemoveEffect", [S(uuid), S(fx)]).unwrap_or(false) }
}

unsafe extern "C" fn srv_entity_clear_effects(_ctx: *mut c_void, uuid: YogStr) -> bool {
    unsafe { bridge!("entityClearEffects", [S(uuid)]).unwrap_or(false) }
}

unsafe extern "C" fn srv_entity_velocity(
    _ctx: *mut c_void,
    uuid: YogStr,
    out: *mut YogVec3,
) -> bool {
    unsafe {
        let Some(s): Option<String> = bridge!("entityVelocity", [S(uuid)]) else {
            return false;
        };
        match parse_tsv::<f64, 3>(&s) {
            Some([x, y, z]) => {
                *out = YogVec3 { x, y, z };
                true
            }
            None => false,
        }
    }
}

unsafe extern "C" fn srv_entity_set_velocity(
    _ctx: *mut c_void,
    uuid: YogStr,
    vel: YogVec3,
) -> bool {
    unsafe {
        bridge!("entitySetVelocity", [S(uuid), D(vel.x), D(vel.y), D(vel.z)]).unwrap_or(false)
    }
}

unsafe extern "C" fn srv_entity_add_velocity(
    _ctx: *mut c_void,
    uuid: YogStr,
    vel: YogVec3,
) -> bool {
    unsafe {
        bridge!("entityAddVelocity", [S(uuid), D(vel.x), D(vel.y), D(vel.z)]).unwrap_or(false)
    }
}

unsafe extern "C" fn srv_has_item_tag(_ctx: *mut c_void, item: YogStr, tag: YogStr) -> bool {
    unsafe { bridge!("hasItemTag", [S(item), S(tag)]).unwrap_or(false) }
}

unsafe extern "C" fn srv_has_block_tag(_ctx: *mut c_void, block: YogStr, tag: YogStr) -> bool {
    unsafe { bridge!("hasBlockTag", [S(block), S(tag)]).unwrap_or(false) }
}

unsafe extern "C" fn srv_drop_loot(
    _ctx: *mut c_void,
    table: YogStr,
    dim: YogStr,
    pos: YogVec3,
) -> bool {
    unsafe {
        bridge!("dropLoot", [S(table), S(dim), D(pos.x), D(pos.y), D(pos.z)]).unwrap_or(false)
    }
}

unsafe extern "C" fn srv_scoreboard_get(
    _ctx: *mut c_void,
    obj: YogStr,
    player: YogStr,
    out: *mut i32,
) -> bool {
    unsafe {
        match bridge!("scoreboardGet", [S(obj), S(player)]) {
            Some(v) if v != i32::MIN => {
                *out = v;
                true
            }
            _ => false,
        }
    }
}

unsafe extern "C" fn srv_scoreboard_set(
    _ctx: *mut c_void,
    obj: YogStr,
    player: YogStr,
    score: i32,
) -> bool {
    unsafe { bridge!("scoreboardSet", [S(obj), S(player), I(score)]).unwrap_or(false) }
}

unsafe extern "C" fn srv_scoreboard_add(
    _ctx: *mut c_void,
    obj: YogStr,
    player: YogStr,
    delta: i32,
    out: *mut i32,
) -> bool {
    unsafe {
        match bridge!("scoreboardAdd", [S(obj), S(player), I(delta)]) {
            Some(v) if v != i32::MIN => {
                *out = v;
                true
            }
            _ => false,
        }
    }
}

unsafe extern "C" fn srv_bossbar_create(
    _ctx: *mut c_void,
    id: YogStr,
    title: YogStr,
    color: YogStr,
    style: YogStr,
) -> bool {
    unsafe { bridge!("bossbarCreate", [S(id), S(title), S(color), S(style)]).unwrap_or(false) }
}

unsafe extern "C" fn srv_bossbar_remove(_ctx: *mut c_void, id: YogStr) -> bool {
    unsafe { bridge!("bossbarRemove", [S(id)]).unwrap_or(false) }
}

unsafe extern "C" fn srv_bossbar_set_title(_ctx: *mut c_void, id: YogStr, title: YogStr) -> bool {
    unsafe { bridge!("bossbarSetTitle", [S(id), S(title)]).unwrap_or(false) }
}

unsafe extern "C" fn srv_bossbar_set_progress(
    _ctx: *mut c_void,
    id: YogStr,
    progress: f32,
) -> bool {
    unsafe { bridge!("bossbarSetProgress", [S(id), F(progress)]).unwrap_or(false) }
}

unsafe extern "C" fn srv_bossbar_set_color(_ctx: *mut c_void, id: YogStr, color: YogStr) -> bool {
    unsafe { bridge!("bossbarSetColor", [S(id), S(color)]).unwrap_or(false) }
}

unsafe extern "C" fn srv_bossbar_add_player(_ctx: *mut c_void, id: YogStr, player: YogStr) -> bool {
    unsafe { bridge!("bossbarAddPlayer", [S(id), S(player)]).unwrap_or(false) }
}

unsafe extern "C" fn srv_bossbar_remove_player(
    _ctx: *mut c_void,
    id: YogStr,
    player: YogStr,
) -> bool {
    unsafe { bridge!("bossbarRemovePlayer", [S(id), S(player)]).unwrap_or(false) }
}

unsafe extern "C" fn srv_bossbar_set_visible(_ctx: *mut c_void, id: YogStr, visible: bool) -> bool {
    unsafe { bridge!("bossbarSetVisible", [S(id), Z(visible)]).unwrap_or(false) }
}

unsafe extern "C" fn srv_get_block_nbt(
    _ctx: *mut c_void,
    dim: YogStr,
    pos: YogBlockPos,
) -> YogOwnedStr {
    unsafe {
        owned(bridge!(
            "getBlockNbt",
            [S(dim), I(pos.x), I(pos.y), I(pos.z)]
        ))
    }
}

unsafe extern "C" fn srv_set_block_nbt(
    _ctx: *mut c_void,
    dim: YogStr,
    pos: YogBlockPos,
    snbt: YogStr,
) -> bool {
    unsafe {
        bridge!(
            "setBlockNbt",
            [S(dim), I(pos.x), I(pos.y), I(pos.z), S(snbt)]
        )
        .unwrap_or(false)
    }
}

unsafe extern "C" fn srv_get_inventory_slot(
    _ctx: *mut c_void,
    dim: YogStr,
    pos: yog_abi::YogBlockPos,
    slot: u32,
) -> YogOwnedStr {
    unsafe {
        owned(bridge!(
            "getInventorySlot",
            [S(dim), I(pos.x), I(pos.y), I(pos.z), I(slot as i32)]
        ))
    }
}

unsafe extern "C" fn srv_set_inventory_slot(
    _ctx: *mut c_void,
    dim: YogStr,
    pos: yog_abi::YogBlockPos,
    slot: u32,
    item_id: YogStr,
    count: u32,
) -> bool {
    unsafe {
        bridge!(
            "setInventorySlot",
            [
                S(dim),
                I(pos.x),
                I(pos.y),
                I(pos.z),
                I(slot as i32),
                S(item_id),
                I(count as i32)
            ]
        )
        .unwrap_or(false)
    }
}

unsafe extern "C" fn srv_player_inventory(_ctx: *mut c_void, player: YogStr) -> YogOwnedStr {
    unsafe { owned(bridge!("playerInventory", [S(player)])) }
}

unsafe extern "C" fn srv_player_set_slot(
    _ctx: *mut c_void,
    player: YogStr,
    slot: u32,
    item_id: YogStr,
    count: u32,
) -> bool {
    unsafe {
        bridge!(
            "playerSetSlot",
            [S(player), I(slot as i32), S(item_id), I(count as i32)]
        )
        .unwrap_or(false)
    }
}

unsafe extern "C" fn srv_player_teleport_dim(
    _ctx: *mut c_void,
    player: YogStr,
    dim: YogStr,
    pos: YogVec3,
) -> bool {
    unsafe {
        bridge!(
            "teleportToDim",
            [S(player), S(dim), D(pos.x), D(pos.y), D(pos.z)]
        )
        .unwrap_or(false)
    }
}

unsafe extern "C" fn srv_entity_teleport_dim(
    _ctx: *mut c_void,
    uuid: YogStr,
    dim: YogStr,
    pos: YogVec3,
) -> bool {
    unsafe {
        bridge!(
            "entityTeleportToDim",
            [S(uuid), S(dim), D(pos.x), D(pos.y), D(pos.z)]
        )
        .unwrap_or(false)
    }
}

unsafe extern "C" fn srv_online_players(_ctx: *mut c_void) -> YogOwnedStr {
    unsafe { owned(bridge!("onlinePlayers", [])) }
}

unsafe extern "C" fn srv_world_entity_count(
    _ctx: *mut c_void,
    dim: YogStr,
    entity_type: YogStr,
) -> i32 {
    unsafe { bridge!("worldEntityCount", [S(dim), S(entity_type)]).unwrap_or(-1) }
}

unsafe extern "C" fn srv_game_dir(_ctx: *mut c_void) -> YogOwnedStr {
    unsafe { owned(bridge!("gameDir", [])) }
}

unsafe extern "C" fn srv_entity_get_nbt(_ctx: *mut c_void, uuid: YogStr) -> YogOwnedStr {
    unsafe { owned(bridge!("entityGetNbt", [S(uuid)])) }
}

unsafe extern "C" fn srv_entity_set_nbt(_ctx: *mut c_void, uuid: YogStr, snbt: YogStr) -> bool {
    unsafe { bridge!("entitySetNbt", [S(uuid), S(snbt)]).unwrap_or(false) }
}

unsafe extern "C" fn srv_spawn_particles(
    _ctx: *mut c_void,
    dim: YogStr,
    pos: YogVec3,
    particle_type: YogStr,
    count: i32,
    dx: f64,
    dy: f64,
    dz: f64,
    speed: f64,
) -> bool {
    unsafe {
        bridge!(
            "spawnParticles",
            [
                S(dim),
                D(pos.x),
                D(pos.y),
                D(pos.z),
                S(particle_type),
                I(count),
                D(dx),
                D(dy),
                D(dz),
                D(speed)
            ]
        )
        .unwrap_or(false)
    }
}

unsafe extern "C" fn srv_entity_attribute_get(
    _ctx: *mut c_void,
    uuid: YogStr,
    attr: YogStr,
) -> f64 {
    unsafe { bridge!("entityAttributeGet", [S(uuid), S(attr)]).unwrap_or(f64::NAN) }
}

unsafe extern "C" fn srv_entity_attribute_set(
    _ctx: *mut c_void,
    uuid: YogStr,
    attr: YogStr,
    value: f64,
) -> bool {
    unsafe { bridge!("entityAttributeSet", [S(uuid), S(attr), D(value)]).unwrap_or(false) }
}

unsafe extern "C" fn srv_get_held_item_nbt(_ctx: *mut c_void, player: YogStr) -> YogOwnedStr {
    unsafe { owned(bridge!("getHeldItemNbt", [S(player)])) }
}

unsafe extern "C" fn srv_set_held_item_nbt(
    _ctx: *mut c_void,
    player: YogStr,
    snbt: YogStr,
) -> bool {
    unsafe { bridge!("setHeldItemNbt", [S(player), S(snbt)]).unwrap_or(false) }
}

unsafe extern "C" fn srv_get_offhand_item_nbt(_ctx: *mut c_void, player: YogStr) -> YogOwnedStr {
    unsafe { owned(bridge!("getOffhandItemNbt", [S(player)])) }
}

unsafe extern "C" fn srv_set_offhand_item_nbt(
    _ctx: *mut c_void,
    player: YogStr,
    snbt: YogStr,
) -> bool {
    unsafe { bridge!("setOffhandItemNbt", [S(player), S(snbt)]).unwrap_or(false) }
}

unsafe extern "C" fn srv_get_slot_item(
    _ctx: *mut c_void,
    player: YogStr,
    slot: u32,
) -> YogOwnedStr {
    unsafe { owned(bridge!("getSlotItem", [S(player), I(slot as i32)])) }
}

unsafe extern "C" fn srv_set_slot_item(
    _ctx: *mut c_void,
    player: YogStr,
    slot: u32,
    item_id: YogStr,
    count: u32,
    snbt: YogStr,
) -> bool {
    unsafe {
        bridge!(
            "setSlotItem",
            [
                S(player),
                I(slot as i32),
                S(item_id),
                I(count as i32),
                S(snbt)
            ]
        )
        .unwrap_or(false)
    }
}

// ── ABI minor 14 — low-level GPU pipeline ────────────────────────────────────
//
// All raw GL calls go through `glow::Context` stored in GL.
// `draw2d_*` functions still call JNI (NativeDraw) for MC text/texture rendering.
// Everything is called on the render thread — no synchronization needed.

// ── helpers ───────────────────────────────────────────────────────────────────

fn gl_draw_mode(mode: u8) -> u32 {
    match mode {
        1 => glow::LINES,
        2 => glow::LINE_STRIP,
        3 => glow::TRIANGLE_STRIP,
        4 => glow::TRIANGLE_FAN,
        _ => glow::TRIANGLES,
    }
}

fn gl_attr_type(dtype: u8) -> u32 {
    match dtype {
        1 => glow::UNSIGNED_BYTE,
        2 => glow::INT,
        3 => glow::UNSIGNED_INT,
        _ => glow::FLOAT,
    }
}

unsafe fn compile_shader(gl: &glow::Context, stage: u32, src: &str) -> Option<glow::NativeShader> {
    unsafe {
        let sh = gl.create_shader(stage).ok()?;
        gl.shader_source(sh, src);
        gl.compile_shader(sh);
        if !gl.get_shader_compile_status(sh) {
            yog_logging::error!("yog-gfx shader compile: {}", gl.get_shader_info_log(sh));
            gl.delete_shader(sh);
            return None;
        }
        Some(sh)
    }
}

// ── GPU buffers ───────────────────────────────────────────────────────────────

unsafe extern "C" fn gfx_buf_create() -> u32 {
    unsafe {
        let Some(g) = GL.get() else { return 0 };
        g.0.create_buffer().map(|b| b.0.get()).unwrap_or(0)
    }
}

unsafe extern "C" fn gfx_buf_delete(handle: u32) {
    unsafe {
        let Some(g) = GL.get() else { return };
        let Some(n) = NonZeroU32::new(handle) else {
            return;
        };
        g.0.delete_buffer(glow::NativeBuffer(n));
    }
}

unsafe extern "C" fn gfx_buf_data(handle: u32, bytes: *const u8, len: u32, dynamic: bool) {
    unsafe {
        let Some(g) = GL.get() else { return };
        let Some(n) = NonZeroU32::new(handle) else {
            return;
        };
        let gl = &g.0;
        let data = std::slice::from_raw_parts(bytes, len as usize);
        let usage = if dynamic {
            glow::DYNAMIC_DRAW
        } else {
            glow::STATIC_DRAW
        };
        gl.bind_buffer(glow::ARRAY_BUFFER, Some(glow::NativeBuffer(n)));
        gl.buffer_data_u8_slice(glow::ARRAY_BUFFER, data, usage);
        gl.bind_buffer(glow::ARRAY_BUFFER, None);
    }
}

unsafe extern "C" fn gfx_buf_subdata(handle: u32, offset: u32, bytes: *const u8, len: u32) {
    unsafe {
        let Some(g) = GL.get() else { return };
        let Some(n) = NonZeroU32::new(handle) else {
            return;
        };
        let gl = &g.0;
        let data = std::slice::from_raw_parts(bytes, len as usize);
        gl.bind_buffer(glow::ARRAY_BUFFER, Some(glow::NativeBuffer(n)));
        gl.buffer_sub_data_u8_slice(glow::ARRAY_BUFFER, offset as i32, data);
        gl.bind_buffer(glow::ARRAY_BUFFER, None);
    }
}

// ── Vertex arrays ─────────────────────────────────────────────────────────────

unsafe extern "C" fn gfx_vao_create() -> u32 {
    unsafe {
        let Some(g) = GL.get() else { return 0 };
        g.0.create_vertex_array().map(|v| v.0.get()).unwrap_or(0)
    }
}

unsafe extern "C" fn gfx_vao_delete(handle: u32) {
    unsafe {
        let Some(g) = GL.get() else { return };
        let Some(n) = NonZeroU32::new(handle) else {
            return;
        };
        g.0.delete_vertex_array(glow::NativeVertexArray(n));
    }
}

unsafe extern "C" fn gfx_vao_attrib(
    vao: u32,
    vbo: u32,
    index: u32,
    components: u8,
    dtype: u8,
    normalized: bool,
    stride: u32,
    offset: u32,
) {
    unsafe {
        let Some(g) = GL.get() else { return };
        let (Some(vn), Some(bn)) = (NonZeroU32::new(vao), NonZeroU32::new(vbo)) else {
            return;
        };
        let gl = &g.0;
        gl.bind_vertex_array(Some(glow::NativeVertexArray(vn)));
        gl.bind_buffer(glow::ARRAY_BUFFER, Some(glow::NativeBuffer(bn)));
        let gl_type = gl_attr_type(dtype);
        if dtype == 2 || dtype == 3 {
            gl.vertex_attrib_pointer_i32(
                index,
                components as i32,
                gl_type,
                stride as i32,
                offset as i32,
            );
        } else {
            gl.vertex_attrib_pointer_f32(
                index,
                components as i32,
                gl_type,
                normalized,
                stride as i32,
                offset as i32,
            );
        }
        gl.enable_vertex_attrib_array(index);
        gl.bind_vertex_array(None);
    }
}

unsafe extern "C" fn gfx_vao_set_ebo(vao: u32, ebo: u32) {
    unsafe {
        let Some(g) = GL.get() else { return };
        let (Some(vn), Some(en)) = (NonZeroU32::new(vao), NonZeroU32::new(ebo)) else {
            return;
        };
        let gl = &g.0;
        gl.bind_vertex_array(Some(glow::NativeVertexArray(vn)));
        gl.bind_buffer(glow::ELEMENT_ARRAY_BUFFER, Some(glow::NativeBuffer(en)));
        gl.bind_vertex_array(None);
    }
}

// ── Shader programs ───────────────────────────────────────────────────────────

/// Returns a path under `~/.cache/yog/shaders/<hash>.ysc` for the given GLSL source pair,
/// creating the directory if needed.  Returns `None` if the home directory is unknown.
fn shader_cache_path(vert: &str, frag: &str) -> Option<PathBuf> {
    use std::hash::{Hash, Hasher};
    let mut h = std::collections::hash_map::DefaultHasher::new();
    vert.hash(&mut h);
    frag.hash(&mut h);
    let hash = h.finish();
    let dir = std::env::var("HOME")
        .map(|home| {
            PathBuf::from(home)
                .join(".cache")
                .join("yog")
                .join("shaders")
        })
        .ok()?;
    std::fs::create_dir_all(&dir).ok()?;
    Some(dir.join(format!("{hash:016x}.ysc")))
}

/// Try restoring a program from a binary blob (`[4 LE bytes: GL format][binary…]`).
/// Returns `Some(handle)` if the driver accepts the binary, `None` otherwise.
///
/// Uses `glProgramBinary` (GL 4.1 / ARB_get_program_binary) via the raw pointer
/// captured in `nativeGlInit`.  Returns `None` if the extension is unavailable.
unsafe fn load_shader_binary(gl: &glow::Context, data: &[u8]) -> Option<glow::NativeProgram> {
    unsafe {
        if data.len() < 4 {
            return None;
        }
        type ProgramBinaryFn = unsafe extern "system" fn(u32, u32, *const c_void, i32);
        let fn_ptr = (*GL_PROGRAM_BINARY.get()?)?;
        let program_binary: ProgramBinaryFn = std::mem::transmute(fn_ptr);

        let fmt = u32::from_le_bytes([data[0], data[1], data[2], data[3]]);
        let prog = gl.create_program().ok()?;
        program_binary(
            prog.0.get(),
            fmt,
            data[4..].as_ptr() as *const c_void,
            (data.len() - 4) as i32,
        );
        if gl.get_program_link_status(prog) {
            Some(prog)
        } else {
            gl.delete_program(prog);
            None
        }
    }
}

/// Read the compiled binary of a linked program.  Returns empty `Vec` if unsupported.
unsafe fn get_shader_binary(prog: glow::NativeProgram) -> (Vec<u8>, u32) {
    unsafe {
        type GetProgramivFn = unsafe extern "system" fn(u32, u32, *mut i32);
        type GetProgramBinaryFn =
            unsafe extern "system" fn(u32, i32, *mut i32, *mut u32, *mut c_void);

        let Some(get_iv_raw) = GL_GET_PROGRAM_IV.get().and_then(|v| *v) else {
            return (vec![], 0);
        };
        let Some(get_bin_raw) = GL_GET_PROGRAM_BINARY.get().and_then(|v| *v) else {
            return (vec![], 0);
        };
        let get_program_iv: GetProgramivFn = std::mem::transmute(get_iv_raw);
        let get_program_binary: GetProgramBinaryFn = std::mem::transmute(get_bin_raw);

        // Query binary size via glGetProgramiv(GL_PROGRAM_BINARY_LENGTH).
        const PROGRAM_BINARY_LENGTH: u32 = 0x8741;
        let mut size: i32 = 0;
        get_program_iv(prog.0.get(), PROGRAM_BINARY_LENGTH, &mut size);
        if size <= 0 {
            return (vec![], 0);
        }

        let mut buf = vec![0u8; size as usize];
        let mut actual_len: i32 = 0;
        let mut fmt: u32 = 0;
        get_program_binary(
            prog.0.get(),
            size,
            &mut actual_len,
            &mut fmt,
            buf.as_mut_ptr() as *mut c_void,
        );
        buf.truncate(actual_len.max(0) as usize);
        (buf, fmt)
    }
}

unsafe extern "C" fn gfx_prog_create(vert: YogStr, frag: YogStr, out: *mut u32) -> bool {
    unsafe {
        let Some(g) = GL.get() else { return false };
        let gl = &g.0;
        let vert_s = vert.as_str();
        let frag_s = frag.as_str();

        // Fast path: binary shader cache — avoids GLSL re-compilation on subsequent launches.
        let cache_path = shader_cache_path(vert_s, frag_s);
        if let Some(ref path) = cache_path
            && let Ok(data) = std::fs::read(path)
        {
            if let Some(prog) = load_shader_binary(gl, &data) {
                *out = prog.0.get();
                return true;
            }
            // Cache stale (driver updated?); remove and fall through to GLSL compile.
            let _ = std::fs::remove_file(path);
        }

        // GLSL compile path.
        let vs = match compile_shader(gl, glow::VERTEX_SHADER, vert_s) {
            Some(s) => s,
            None => return false,
        };
        let fs = match compile_shader(gl, glow::FRAGMENT_SHADER, frag_s) {
            Some(s) => s,
            None => {
                gl.delete_shader(vs);
                return false;
            }
        };
        let prog = match gl.create_program() {
            Ok(p) => p,
            Err(e) => {
                yog_logging::error!("yog-gfx: create_program: {}", e);
                gl.delete_shader(vs);
                gl.delete_shader(fs);
                return false;
            }
        };
        gl.attach_shader(prog, vs);
        gl.attach_shader(prog, fs);
        gl.link_program(prog);
        gl.detach_shader(prog, vs);
        gl.detach_shader(prog, fs);
        gl.delete_shader(vs);
        gl.delete_shader(fs);
        if !gl.get_program_link_status(prog) {
            yog_logging::error!("yog-gfx: shader link: {}", gl.get_program_info_log(prog));
            gl.delete_program(prog);
            return false;
        }

        // Persist binary so the next launch skips GLSL compilation entirely.
        if let Some(ref path) = cache_path {
            let (binary, fmt) = get_shader_binary(prog);
            if !binary.is_empty() {
                let mut blob = fmt.to_le_bytes().to_vec();
                blob.extend_from_slice(&binary);
                let _ = std::fs::write(path, &blob);
            }
        }

        *out = prog.0.get();
        true
    }
}

unsafe extern "C" fn gfx_prog_delete(handle: u32) {
    unsafe {
        let Some(g) = GL.get() else { return };
        let Some(n) = NonZeroU32::new(handle) else {
            return;
        };
        g.0.delete_program(glow::NativeProgram(n));
    }
}

macro_rules! with_prog {
    ($handle:expr_2021, |$gl:ident, $prog:ident, $loc:ident ($name:expr_2021)| $body:expr_2021) => {{
        let Some(g) = GL.get() else { return };
        let Some(n) = NonZeroU32::new($handle) else {
            return;
        };
        let $gl = &g.0;
        let $prog = glow::NativeProgram(n);
        $gl.use_program(Some($prog));
        let $loc = $gl.get_uniform_location($prog, $name.as_str());
        $body
    }};
}

unsafe extern "C" fn gfx_prog_uniform_1i(prog: u32, name: YogStr, v: i32) {
    unsafe {
        with_prog!(prog, |gl, _p, loc(name)| gl.uniform_1_i32(loc.as_ref(), v));
    }
}
unsafe extern "C" fn gfx_prog_uniform_1f(prog: u32, name: YogStr, v: f32) {
    unsafe {
        with_prog!(prog, |gl, _p, loc(name)| gl.uniform_1_f32(loc.as_ref(), v));
    }
}
unsafe extern "C" fn gfx_prog_uniform_2f(prog: u32, name: YogStr, x: f32, y: f32) {
    unsafe {
        with_prog!(prog, |gl, _p, loc(name)| gl.uniform_2_f32(
            loc.as_ref(),
            x,
            y
        ));
    }
}
unsafe extern "C" fn gfx_prog_uniform_3f(prog: u32, name: YogStr, x: f32, y: f32, z: f32) {
    unsafe {
        with_prog!(prog, |gl, _p, loc(name)| gl.uniform_3_f32(
            loc.as_ref(),
            x,
            y,
            z
        ));
    }
}
unsafe extern "C" fn gfx_prog_uniform_4f(prog: u32, name: YogStr, x: f32, y: f32, z: f32, w: f32) {
    unsafe {
        with_prog!(prog, |gl, _p, loc(name)| gl.uniform_4_f32(
            loc.as_ref(),
            x,
            y,
            z,
            w
        ));
    }
}
unsafe extern "C" fn gfx_prog_uniform_mat4(prog: u32, name: YogStr, col_major: *const f32) {
    unsafe {
        with_prog!(prog, |gl, _p, loc(name)| {
            let data = std::slice::from_raw_parts(col_major, 16);
            gl.uniform_matrix_4_f32_slice(loc.as_ref(), false, data);
        });
    }
}

// ── Textures ──────────────────────────────────────────────────────────────────

unsafe extern "C" fn gfx_tex_create(w: u32, h: u32, rgba: *const u8, linear: bool) -> u32 {
    unsafe {
        let Some(g) = GL.get() else { return 0 };
        let gl = &g.0;
        let tex = match gl.create_texture() {
            Ok(t) => t,
            Err(_) => return 0,
        };
        gl.bind_texture(glow::TEXTURE_2D, Some(tex));
        let filter = if linear { glow::LINEAR } else { glow::NEAREST };
        gl.tex_parameter_i32(glow::TEXTURE_2D, glow::TEXTURE_MIN_FILTER, filter as i32);
        gl.tex_parameter_i32(glow::TEXTURE_2D, glow::TEXTURE_MAG_FILTER, filter as i32);
        gl.tex_parameter_i32(
            glow::TEXTURE_2D,
            glow::TEXTURE_WRAP_S,
            glow::CLAMP_TO_EDGE as i32,
        );
        gl.tex_parameter_i32(
            glow::TEXTURE_2D,
            glow::TEXTURE_WRAP_T,
            glow::CLAMP_TO_EDGE as i32,
        );
        let pixels = std::slice::from_raw_parts(rgba, (w * h * 4) as usize);
        // The game leaves its own unpacking (row length, skips) set from its
        // atlas uploads: read with those, a tight buffer is overrun.
        unpack_tightly(gl);
        gl.tex_image_2d(
            glow::TEXTURE_2D,
            0,
            glow::RGBA8 as i32,
            w as i32,
            h as i32,
            0,
            glow::RGBA,
            glow::UNSIGNED_BYTE,
            glow::PixelUnpackData::Slice(Some(pixels)),
        );
        gl.bind_texture(glow::TEXTURE_2D, None);
        tex.0.get()
    }
}

/// Pixel unpacking as a tightly packed client buffer needs it.
unsafe fn unpack_tightly(gl: &glow::Context) {
    unsafe {
        gl.bind_buffer(glow::PIXEL_UNPACK_BUFFER, None);
        gl.pixel_store_i32(glow::UNPACK_ROW_LENGTH, 0);
        gl.pixel_store_i32(glow::UNPACK_SKIP_ROWS, 0);
        gl.pixel_store_i32(glow::UNPACK_SKIP_PIXELS, 0);
        gl.pixel_store_i32(glow::UNPACK_ALIGNMENT, 4);
    }
}

unsafe extern "C" fn gfx_tex_delete(handle: u32) {
    unsafe {
        let Some(g) = GL.get() else { return };
        let Some(n) = NonZeroU32::new(handle) else {
            return;
        };
        g.0.delete_texture(glow::NativeTexture(n));
    }
}

unsafe extern "C" fn gfx_tex_bind(unit: u32, handle: u32) {
    unsafe {
        let Some(g) = GL.get() else { return };
        let gl = &g.0;
        gl.active_texture(glow::TEXTURE0 + unit);
        match NonZeroU32::new(handle) {
            Some(n) => gl.bind_texture(glow::TEXTURE_2D, Some(glow::NativeTexture(n))),
            None => gl.bind_texture(glow::TEXTURE_2D, None),
        }
    }
}

unsafe extern "C" fn gfx_tex_from_mc(id: YogStr) -> u32 {
    unsafe { host!("dev/yog/NativeDraw", "getMcTextureId", [S(id)]).map_or(0, |id: i32| id as u32) }
}

// ── Draw calls ────────────────────────────────────────────────────────────────

unsafe extern "C" fn gfx_draw_arrays(vao: u32, prog: u32, mode: u8, first: u32, count: u32) {
    unsafe {
        let Some(g) = GL.get() else { return };
        let (Some(vn), Some(pn)) = (NonZeroU32::new(vao), NonZeroU32::new(prog)) else {
            return;
        };
        let gl = &g.0;
        gl.use_program(Some(glow::NativeProgram(pn)));
        gl.bind_vertex_array(Some(glow::NativeVertexArray(vn)));
        gl.draw_arrays(gl_draw_mode(mode), first as i32, count as i32);
        gl.bind_vertex_array(None);
    }
}

unsafe extern "C" fn gfx_draw_elements(
    vao: u32,
    ebo: u32,
    prog: u32,
    mode: u8,
    count: u32,
    u32_idx: bool,
) {
    unsafe {
        let Some(g) = GL.get() else { return };
        let (Some(vn), Some(pn)) = (NonZeroU32::new(vao), NonZeroU32::new(prog)) else {
            return;
        };
        let gl = &g.0;
        gl.use_program(Some(glow::NativeProgram(pn)));
        gl.bind_vertex_array(Some(glow::NativeVertexArray(vn)));
        // EBO is stored in the VAO; ebo param is informational for safety but not re-bound here.
        let _ = ebo;
        let idx_type = if u32_idx {
            glow::UNSIGNED_INT
        } else {
            glow::UNSIGNED_SHORT
        };
        gl.draw_elements(gl_draw_mode(mode), count as i32, idx_type, 0);
        gl.bind_vertex_array(None);
    }
}

// ── Render state ──────────────────────────────────────────────────────────────

unsafe extern "C" fn gfx_set_blend(enabled: bool, src: u32, dst: u32) {
    unsafe {
        let Some(g) = GL.get() else { return };
        let gl = &g.0;
        if enabled {
            gl.enable(glow::BLEND);
            gl.blend_func(src, dst);
        } else {
            gl.disable(glow::BLEND);
        }
    }
}

unsafe extern "C" fn gfx_set_depth(test: bool, write: bool) {
    unsafe {
        let Some(g) = GL.get() else { return };
        let gl = &g.0;
        if test {
            gl.enable(glow::DEPTH_TEST);
        } else {
            gl.disable(glow::DEPTH_TEST);
        }
        gl.depth_mask(write);
        // Disable face culling: our 2D quads use CW winding (back-face) due to y-flip.
        if !test {
            gl.disable(glow::CULL_FACE);
        }
    }
}

unsafe extern "C" fn gfx_set_scissor(x: i32, y: i32, w: i32, h: i32) {
    unsafe {
        let Some(g) = GL.get() else { return };
        let gl = &g.0;
        gl.enable(glow::SCISSOR_TEST);
        gl.scissor(x, y, w, h);
    }
}

unsafe extern "C" fn gfx_clear_scissor() {
    unsafe {
        let Some(g) = GL.get() else { return };
        g.0.disable(glow::SCISSOR_TEST);
    }
}

unsafe extern "C" fn gfx_set_viewport(x: i32, y: i32, w: i32, h: i32) {
    unsafe {
        let Some(g) = GL.get() else { return };
        g.0.viewport(x, y, w, h);
    }
}

// ── 2D convenience (JNI — uses MC's DrawContext / text renderer) ──────────────

unsafe extern "C" fn gfx_draw2d_rect(x1: f32, y1: f32, x2: f32, y2: f32, color: u32) {
    unsafe {
        let _: Option<()> = host!(
            "dev/yog/NativeDraw",
            "drawRect",
            [F(x1), F(y1), F(x2), F(y2), I(color as i32)]
        );
    }
}

unsafe extern "C" fn gfx_draw2d_gradient(
    x1: f32,
    y1: f32,
    x2: f32,
    y2: f32,
    top: u32,
    bottom: u32,
) {
    unsafe {
        let _: Option<()> = host!(
            "dev/yog/NativeDraw",
            "drawGradientRect",
            [F(x1), F(y1), F(x2), F(y2), I(top as i32), I(bottom as i32)]
        );
    }
}

unsafe extern "C" fn gfx_draw2d_text(text: YogStr, x: f32, y: f32, color: u32, shadow: bool) {
    unsafe {
        let _: Option<()> = host!(
            "dev/yog/NativeDraw",
            "drawText",
            [S(text), F(x), F(y), I(color as i32), Z(shadow)]
        );
    }
}

unsafe extern "C" fn gfx_draw2d_text_font(
    text: YogStr,
    font: YogStr,
    x: f32,
    y: f32,
    color: u32,
    shadow: bool,
) {
    unsafe {
        let _: Option<()> = host!(
            "dev/yog/NativeDraw",
            "drawTextFont",
            [S(text), S(font), F(x), F(y), I(color as i32), Z(shadow)]
        );
    }
}

unsafe extern "C" fn gfx_draw2d_text_scaled(
    text: YogStr,
    font: YogStr,
    x: f32,
    y: f32,
    scale: f32,
    color: u32,
    shadow: bool,
) {
    unsafe {
        let _: Option<()> = host!(
            "dev/yog/NativeDraw",
            "drawTextScaled",
            [S(text), S(font), F(x), F(y), F(scale), I(color as i32), Z(shadow)]
        );
    }
}

unsafe extern "C" fn gfx_draw2d_blocks(blocks: YogStr, x: f32, y: f32, w: f32, h: f32, yaw: f32, pitch: f32) {
    if w <= 0.0 || h <= 0.0 {
        return;
    }
    unsafe {
        let _: Option<()> = host!(
            "dev/yog/NativeDraw",
            "drawBlocks",
            [S(blocks), F(x), F(y), F(w), F(h), F(yaw), F(pitch)]
        );
    }
}

unsafe extern "C" fn gfx_text_width(text: YogStr, font: YogStr) -> f32 {
    unsafe { host!("dev/yog/NativeDraw", "textWidth", [S(text), S(font)]).unwrap_or(0.0) }
}

unsafe extern "C" fn gfx_draw2d_item(id: YogStr, x: f32, y: f32, size: f32) {
    unsafe {
        let _: Option<()> = host!(
            "dev/yog/NativeDraw",
            "drawItem",
            [S(id), F(x), F(y), F(size)]
        );
    }
}

// ── Static GFX function table (function pointers only — per-frame data filled at call time) ──

static GFX_FN_TABLE: YogGfxApi = YogGfxApi {
    // Per-frame fields zeroed in the static; actual values are set on the stack per render call.
    screen_w: 0,
    screen_h: 0,
    delta_tick: 0.0,
    scale_factor: 1.0,
    view_proj: [0.0; 16],
    camera_pos: [0.0; 3],
    player_pos: [0.0; 3],
    _pad1: 0.0,
    buf_create: gfx_buf_create,
    buf_delete: gfx_buf_delete,
    buf_data: gfx_buf_data,
    buf_subdata: gfx_buf_subdata,
    vao_create: gfx_vao_create,
    vao_delete: gfx_vao_delete,
    vao_attrib: gfx_vao_attrib,
    vao_set_ebo: gfx_vao_set_ebo,
    prog_create: gfx_prog_create,
    prog_delete: gfx_prog_delete,
    prog_uniform_1i: gfx_prog_uniform_1i,
    prog_uniform_1f: gfx_prog_uniform_1f,
    prog_uniform_2f: gfx_prog_uniform_2f,
    prog_uniform_3f: gfx_prog_uniform_3f,
    prog_uniform_4f: gfx_prog_uniform_4f,
    prog_uniform_mat4: gfx_prog_uniform_mat4,
    tex_create: gfx_tex_create,
    tex_delete: gfx_tex_delete,
    tex_bind: gfx_tex_bind,
    tex_from_mc: gfx_tex_from_mc,
    draw_arrays: gfx_draw_arrays,
    draw_elements: gfx_draw_elements,
    set_blend: gfx_set_blend,
    set_depth: gfx_set_depth,
    set_scissor: gfx_set_scissor,
    clear_scissor: gfx_clear_scissor,
    set_viewport: gfx_set_viewport,
    draw2d_rect: gfx_draw2d_rect,
    draw2d_gradient: gfx_draw2d_gradient,
    draw2d_text: gfx_draw2d_text,
    draw2d_item: gfx_draw2d_item,
    draw2d_text_font: gfx_draw2d_text_font,
    text_width: gfx_text_width,
    draw2d_image: images::gfx_draw2d_image,
    image_size: images::gfx_image_size,
    draw2d_svg: svg::gfx_draw2d_svg,
    draw2d_entity: svg::gfx_draw2d_entity,
    draw2d_text_scaled: gfx_draw2d_text_scaled,
    draw2d_blocks: gfx_draw2d_blocks,
};

// ── YogApi registration functions ─────────────────────────────────────────────
//
// ctx is *const RwLock<RuntimeHandlers> (cast from *mut c_void) — see
// `current_registration()` for how each entry gets tagged with the
// `LoadedModule` that's registering it.

macro_rules! api_event {
    ($name:ident, $field:ident, $fn_ty:ty) => {
        unsafe extern "C" fn $name(ctx: *mut c_void, ud: *mut c_void, h: $fn_ty) {
            unsafe {
                let mut handlers = handlers_mut(ctx);
                handlers.$field.push((current_registration(), ud, h));
            }
        }
    };
}

api_event!(api_on_block_break, block_break, YogBlockBreakFn);
api_event!(api_on_chat, chat, YogChatFn);
api_event!(api_on_player_join, player_join, YogPlayerFn);
api_event!(api_on_player_leave, player_leave, YogPlayerFn);
api_event!(api_on_use_item, use_item, YogUseItemFn);
api_event!(api_on_use_block, use_block, YogUseBlockFn);
api_event!(api_on_attack_entity, attack_entity, YogAttackEntityFn);
api_event!(api_on_entity_damage, entity_damage, YogEntityDamageFn);
api_event!(api_on_entity_death, entity_death, YogEntityDeathFn);
api_event!(api_on_entity_spawn, entity_spawn, YogEntitySpawnFn);
api_event!(
    api_on_player_place_block,
    player_place_block,
    YogPlaceBlockFn
);
api_event!(api_on_player_death, player_death, YogPlayerDeathFn);
api_event!(api_on_player_respawn, player_respawn, YogPlayerRespawnFn);
api_event!(api_on_advancement, advancement, YogAdvancementFn);
api_event!(api_on_entity_interact, entity_interact, YogEntityInteractFn);
api_event!(api_on_item_craft, item_craft, YogCraftFn);
api_event!(api_on_machine_craft, machine_craft, YogMachineCraftFn);
api_event!(api_on_explosion, explosion, YogExplosionFn);
api_event!(api_on_item_pickup, item_pickup, YogItemPickupFn);
api_event!(api_on_item_insert, item_insert, YogItemInsertFn);
api_event!(api_on_player_move, player_move, YogPlayerMoveFn);
api_event!(api_on_container_open, container_open, YogContainerOpenFn);
api_event!(api_on_container_close, container_close, YogContainerCloseFn);
api_event!(api_on_projectile_hit, projectile_hit, YogProjectileHitFn);
api_event!(api_on_client_tick, client_tick, YogClientFn);
api_event!(api_on_hud_render, hud_render, YogHudRenderFn);
api_event!(api_on_world_render, world_render, YogWorldRenderFn);
api_event!(api_on_key_press, key_press, YogKeyPressFn);
api_event!(api_on_screen_open, screen_open, YogScreenFn);
api_event!(api_on_screen_close, screen_close, YogScreenFn);
api_event!(api_on_server_tick, server_tick, YogServerFn);
api_event!(api_on_server_started, server_started, YogServerFn);
api_event!(api_on_server_stopping, server_stopping, YogServerFn);

unsafe extern "C" fn api_on_packet(
    ctx: *mut c_void,
    channel: YogStr,
    ud: *mut c_void,
    h: YogPacketFn,
) {
    unsafe {
        let mut handlers = handlers_mut(ctx);
        handlers
            .packets
            .insert(channel.as_str().to_owned(), (current_registration(), ud, h));
    }
}

unsafe extern "C" fn api_on_client_packet(
    ctx: *mut c_void,
    channel: YogStr,
    ud: *mut c_void,
    h: YogPacketFn,
) {
    unsafe {
        let mut handlers = handlers_mut(ctx);
        handlers
            .client_packets
            .insert(channel.as_str().to_owned(), (current_registration(), ud, h));
    }
}

unsafe extern "C" fn api_register_command(
    ctx: *mut c_void,
    name: YogStr,
    ud: *mut c_void,
    h: YogCommandFn,
) {
    unsafe {
        let mut handlers = handlers_mut(ctx);
        handlers
            .commands
            .entry(name.as_str().to_owned())
            .or_default()
            .push((current_registration(), None, ud, h));
    }
}

unsafe extern "C" fn api_register_typed_command(
    ctx: *mut c_void,
    name: YogStr,
    schema: YogStr,
    ud: *mut c_void,
    h: YogCommandFn,
) {
    unsafe {
        let mut handlers = handlers_mut(ctx);
        let n = name.as_str().to_owned();
        handlers.commands.entry(n).or_default().push((
            current_registration(),
            Some(schema.as_str().to_owned()),
            ud,
            h,
        ));
    }
}

/// The handlers a registration call writes into: `ctx` is the
/// `RwLock<RuntimeHandlers>` the runtime handed the mod's `yog_mod_register`.
/// A poisoned lock (a mod panicked mid-registration) is taken as it is —
/// one mod's panic doesn't stop the others registering.
///
/// # Safety
/// `ctx` must be that registration context.
unsafe fn handlers_mut<'a>(ctx: *mut c_void) -> std::sync::RwLockWriteGuard<'a, RuntimeHandlers> {
    unsafe { (*(ctx as *const RwLock<RuntimeHandlers>)).write().unwrap_or_else(|e| e.into_inner()) }
}

/// `register_content`: content of `kind` named `id`, its definition a Yog
/// Data blob — the one way mods hand the runtime what the hosts register.
unsafe extern "C" fn api_register_content(ctx: *mut c_void, kind: YogStr, id: YogStr, data: *const u8, len: usize) {
    unsafe {
        let kind = kind.as_str();
        let id = id.as_str().to_owned();
        let bytes = if data.is_null() || len == 0 { &[][..] } else { std::slice::from_raw_parts(data, len) };
        // Borrowed: strings stay in `bytes`, only what's kept is copied.
        let arena = yog_data::Bump::new();
        let value = match yog_data::binary::decode_ref(bytes, &arena) {
            Ok(v) => v,
            Err(e) => {
                yog_logging::warn!("{kind} {id}: not Yog Data: {e}");
                return;
            }
        };
        register_content(&mut handlers_mut(ctx), kind, id, &value);
    }
}

/// Stores content of `kind` (see `register_content` in yog-abi).
fn register_content(h: &mut RuntimeHandlers, kind: &str, id: String, value: &yog_data::ValueRef) {
    *h.content_blob.get_mut().unwrap_or_else(|e| e.into_inner()) = None;
    fn read<T: yog_data::Schema>(kind: &str, id: &str, v: &yog_data::ValueRef) -> Option<T> {
        match yog_data::read_ref_value::<T>(v) {
            Ok(value) => Some(value),
            Err(problems) => {
                let list: Vec<String> = problems.iter().map(|p| format!("{}: {}", p.path, p.message)).collect();
                yog_logging::warn!("{kind} {id}: {}", list.join("; "));
                None
            }
        }
    }
    let none_if_empty = |s: &mut Option<String>| {
        if s.as_deref() == Some("") {
            *s = None;
        }
    };
    match kind {
        "item" => {
            if let Some(mut d) = read::<ItemDef>(kind, &id, value) {
                none_if_empty(&mut d.name);
                none_if_empty(&mut d.tooltip);
                h.items.push(d);
            }
        }
        "block" => {
            if let Some(mut d) = read::<BlockDef>(kind, &id, value) {
                none_if_empty(&mut d.name);
                none_if_empty(&mut d.sound);
                none_if_empty(&mut d.inventory_id);
                h.blocks.push(d);
            }
        }
        "entity" => {
            if read::<yog_registry::EntityDef>(kind, &id, value).is_some() {
                h.entities.push(value.to_value());
            }
        }
        "biome_feature" => {
            if let Some(f) = read::<yog_registry::BiomeFeature>(kind, &id, value) {
                h.biome_features.push(f);
            }
        }
        "tool_material" => {
            if let Some(m) = read::<yog_registry::ToolMaterial>(kind, &id, value) {
                h.tool_materials.insert(m.id.clone(), m);
            }
        }
        "armor_material" => {
            if let Some(m) = read::<yog_registry::ArmorMaterial>(kind, &id, value) {
                h.armor_materials.insert(m.id.clone(), m);
            }
        }
        "sound_group" => {
            if let Some(g) = read::<yog_registry::SoundGroup>(kind, &id, value) {
                h.sound_groups.insert(g.id.clone(), g);
            }
        }
        "inventory" => {
            if let Some(d) = read::<yog_inventory::InventoryDef>(kind, &id, value) {
                h.inventories.push(d);
            }
        }
        "startup_grant" => {
            if let Some(mut g) = read::<yog_registry::StartupGrant>(kind, &id, value) {
                none_if_empty(&mut g.book);
                none_if_empty(&mut g.command);
                h.startup_grants.push(g);
            }
        }
        "recipe" => {
            let (ns, name) = id.split_once(':').unwrap_or(("minecraft", &id));
            let recipe = value.to_value();
            // Feed the recipe to the books already registered (their recipe pages).
            for r in h.book_renderers.lock().unwrap().values_mut() {
                r.add_recipe(format!("{ns}:{name}"), &recipe);
            }
            h.recipes.push((ns.to_string(), name.to_string(), recipe));
        }
        "book" => {
            // Read by its schema; the renderer draws it without a round trip to Java.
            if let Some(mut book) = read::<yog_book::Book>(kind, &id, value) {
                // Entries about mods that aren't here aren't shown.
                book.entries.retain(|e| e.requires.iter().all(|m| mod_loaded(m)));
                h.books.insert(id.clone());
                let mut renderer = yog_book::BookRenderer::new(book);
                // i18n books carry translation keys; shown in the game language.
                renderer.set_translator(translate);
                // Recipes may register before the book — replay them into the renderer.
                for (ns, name, recipe) in &h.recipes {
                    renderer.add_recipe(format!("{ns}:{name}"), recipe);
                }
                h.book_renderers.get_mut().unwrap_or_else(|e| e.into_inner()).insert(id, renderer);
            }
        }
        "dimension" => {
            yog_logging::info!("registered dimension type: {}", id);
            let mut def = value.to_value();
            // The terrain stays here; the host gets the dimension, its generator this one.
            if let Some(t) = value.get("terrain") {
                let Some(terrain) = read::<yog_worldgen::Terrain>(kind, &id, t) else { return };
                if let yog_data::Value::Map(m) = &mut def {
                    m.remove("terrain");
                    m.insert("generator_type", yog_data::Value::from(id.as_str()));
                }
                h.terrains.insert(id.clone(), Arc::new(yog_worldgen::Generator::new(terrain)));
            }
            h.dimensions.insert(id, def);
        }
        other => yog_logging::warn!("unknown kind of content `{other}` ({id}) — this runtime doesn't know it"),
    }
}




unsafe extern "C" fn api_schedule_once(
    ctx: *mut c_void,
    delay_ticks: u64,
    ud: *mut c_void,
    h: YogScheduledFn,
) {
    unsafe {
        let handlers = handlers_mut(ctx);
        handlers
            .scheduler
            .lock()
            .expect("scheduler poisoned")
            .once_tasks
            .push(OnceTask {
                module: current_registration(),
                delay_remaining: delay_ticks,
                ud,
                f: h,
            });
    }
}

unsafe extern "C" fn api_schedule_repeating(
    ctx: *mut c_void,
    period_ticks: u64,
    ud: *mut c_void,
    h: YogScheduledFn,
) {
    unsafe {
        let handlers = handlers_mut(ctx);
        handlers
            .scheduler
            .lock()
            .expect("scheduler poisoned")
            .repeating_tasks
            .push(RepeatingTask {
                module: current_registration(),
                period: period_ticks,
                ticks_left: period_ticks.max(1),
                ud,
                f: h,
            });
    }
}




unsafe extern "C" fn api_register_chunk_generator(
    ctx: *mut c_void,
    id: YogStr,
    ud: *mut c_void,
    h: yog_abi::YogChunkGeneratorFn,
) {
    unsafe {
        let mut handlers = handlers_mut(ctx);
        let id = id.as_str().to_owned();
        yog_logging::info!("registered chunk generator for dimension: {}", id);
        handlers
            .chunk_generators
            .insert(id, (current_registration(), ud, h));
    }
}

unsafe extern "C" fn api_register_ui(
    ctx: *mut c_void,
    ui_id: YogStr,
    _layout_json: YogStr,
    ud: *mut c_void,
    h: yog_abi::YogUIEventFn,
) {
    unsafe {
        let mut handlers = handlers_mut(ctx);
        let id = ui_id.as_str().to_owned();
        handlers
            .ui_handlers
            .insert(id.clone(), (current_registration(), ud, h));
        yog_logging::info!("registered UI handler: {}", id);
    }
}

unsafe extern "C" fn api_on_ui_render(
    ctx: *mut c_void,
    ui_id: YogStr,
    ud: *mut c_void,
    h: YogHudRenderFn,
) {
    unsafe {
        let mut handlers = handlers_mut(ctx);
        let id = ui_id.as_str().to_owned();
        handlers
            .ui_render_handlers
            .entry(id)
            .or_default()
            .push((current_registration(), ud, h));
    }
}

unsafe extern "C" fn api_on_book_page(
    ctx: *mut c_void,
    drawer: YogStr,
    ud: *mut c_void,
    f: yog_abi::YogPageDrawFn,
) {
    unsafe {
        let mut handlers = handlers_mut(ctx);
        handlers
            .page_drawers
            .insert(drawer.as_str().to_owned(), (current_registration(), ud, f));
    }
}

unsafe extern "C" fn api_on_item_tooltip(
    ctx: *mut c_void,
    item: YogStr,
    ud: *mut c_void,
    size: yog_abi::YogTooltipSizeFn,
    draw: yog_abi::YogTooltipDrawFn,
) {
    unsafe {
        let mut handlers = handlers_mut(ctx);
        handlers
            .item_tooltips
            .entry(item.as_str().to_owned())
            .or_default()
            .push((current_registration(), ud, size, draw));
    }
}

unsafe extern "C" fn api_on_entity_tick(ctx: *mut c_void, entity_type: YogStr, ud: *mut c_void, f: yog_abi::YogEntityTickFn) {
    unsafe {
        handlers_mut(ctx).entity_ticks.entry(entity_type.as_str().to_owned()).or_default().push((current_registration(), ud, f));
    }
}

/// `nativeOnEntityTick(String type, String uuid)`: a tick of a mod's entity.
#[unsafe(no_mangle)]
pub extern "system" fn Java_dev_yog_NativeBridge_nativeOnEntityTick<'l>(
    mut env: JNIEnv<'l>,
    _class: JClass<'l>,
    entity_type: JString<'l>,
    uuid: JString<'l>,
) {
    let (t, u) = (jstr!(env, entity_type), jstr!(env, uuid));
    let entries = handlers().entity_ticks.get(&t).cloned().unwrap_or_default();
    let srv = srv_ptr();
    guard("on_entity_tick", || each(&entries, |ud, f| unsafe { f(ud, srv, YogStr::from_str(&u)) }));
}

unsafe extern "C" fn api_on_item_text(ctx: *mut c_void, item: YogStr, ud: *mut c_void, f: yog_abi::YogItemTextFn) {
    unsafe {
        handlers_mut(ctx).item_texts.entry(item.as_str().to_owned()).or_default().push((current_registration(), ud, f));
    }
}

unsafe extern "C" fn copy_text(sink: *mut c_void, text: YogStr) {
    // SAFETY: `sink` is the `String` `item_text` passes; `text` is valid for the call.
    unsafe { (*(sink as *mut String)).push_str(text.as_str()) }
}

/// A stack's own name and tooltip lines (`name\nline…`), from the first
/// mod that gives them; `None`: the item's own.
fn item_text(item: &str, snbt: &str) -> Option<String> {
    let entries = handlers().item_texts.get(item).cloned().unwrap_or_default();
    for (module, ud, f) in entries {
        let Some(_call) = module.enter() else { continue };
        let mut out = String::new();
        let mut given = false;
        guard("item_text", || unsafe {
            given = f(ud, YogStr::from_str(item), YogStr::from_str(snbt), &mut out as *mut String as *mut c_void, copy_text);
        });
        if given {
            return Some(out);
        }
    }
    None
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_dev_yog_NativeBridge_nativeItemText<'l>(
    mut env: JNIEnv<'l>,
    _class: JClass<'l>,
    item: JString<'l>,
    snbt: JString<'l>,
) -> jstring {
    let item: String = env.get_string(&item).map(String::from).unwrap_or_default();
    let snbt: String = env.get_string(&snbt).map(String::from).unwrap_or_default();
    match item_text(&item, &snbt) {
        Some(t) => env.new_string(t).map(JString::into_raw).unwrap_or(std::ptr::null_mut()),
        None => std::ptr::null_mut(),
    }
}

// ── Machines (ABI 2.5) ───────────────────────────────────────────────────────

/// A machine's event about recipe `found`.
fn machine_event<'a>(
    m: &'a yog_inventory::Machine,
    def: &'a str,
    found: &'a machine::Found,
    dim: &'a str,
    pos: YogBlockPos,
    player: &'a str,
) -> YogMachineCraftEvent {
    YogMachineCraftEvent {
        machine: YogStr::from_str(def),
        kind: YogStr::from_str(&m.kind),
        recipe: YogStr::from_str(&found.recipe),
        result_item: YogStr::from_str(&found.result),
        result_count: found.count,
        values: YogStr::from_str(&found.values),
        pos,
        dimension: YogStr::from_str(dim),
        player: YogStr::from_str(player),
    }
}

/// The recipe machine `def` at `pos` makes of `items` (by inventory slot,
/// `""` empty): the first that fits and every mod allows; `recipe` only it.
fn machine_recipe(def: &str, dim: &str, pos: YogBlockPos, items: &[String], recipe: Option<&str>) -> Option<machine::Found> {
    let h = handlers();
    let m = h.inventories.iter().find(|d| d.id == def)?.machine.as_ref()?;
    let has_tag = |item: &str, tag: &str| unsafe { srv_has_item_tag(std::ptr::null_mut(), YogStr::from_str(item), YogStr::from_str(tag)) };
    let recipes = h.recipes.iter().map(|(ns, name, r)| (format!("{ns}:{name}"), r)).filter(|(id, _)| recipe.is_none_or(|r| r == id));
    let srv = srv_ptr();
    machine::find(m, recipes, items, &has_tag).find(|found| {
        let ev = machine_event(m, def, found, dim, pos, "");
        allowed(&h.machine_craft, |ud, f| unsafe { f(ud, srv, &ev, 0) })
    })
}

/// What machine `def` at `x y z` makes of `items` (`\n`-joined, by slot):
/// `recipe\nresult\ncount`, or null.
#[unsafe(no_mangle)]
pub extern "system" fn Java_dev_yog_NativeBridge_nativeMachineResult<'l>(
    mut env: JNIEnv<'l>,
    _class: JClass<'l>,
    def: JString<'l>,
    dimension: JString<'l>,
    x: jint,
    y: jint,
    z: jint,
    items: JString<'l>,
) -> jstring {
    let (def, d, items) = (jstr!(env, def, std::ptr::null_mut()), jstr!(env, dimension, std::ptr::null_mut()), jstr!(env, items, std::ptr::null_mut()));
    let items: Vec<String> = items.split('\n').map(str::to_string).collect();
    let mut found = None;
    guard("machine_result", || found = machine_recipe(&def, &d, YogBlockPos { x, y, z }, &items, None));
    match found {
        Some(f) => env.new_string(format!("{}\n{}\n{}", f.recipe, f.result, f.count)).map(JString::into_raw).unwrap_or(std::ptr::null_mut()),
        None => std::ptr::null_mut(),
    }
}

/// `player` took machine `def`'s result, made by `recipe` of `items` (the
/// ingredients still in): whether it may — then the mods take their share.
#[unsafe(no_mangle)]
#[allow(clippy::too_many_arguments)]
pub extern "system" fn Java_dev_yog_NativeBridge_nativeMachineCraft<'l>(
    mut env: JNIEnv<'l>,
    _class: JClass<'l>,
    def: JString<'l>,
    recipe: JString<'l>,
    dimension: JString<'l>,
    x: jint,
    y: jint,
    z: jint,
    items: JString<'l>,
    player: JString<'l>,
) -> jni::sys::jboolean {
    let (def, recipe, d, items, player) =
        (jstr!(env, def, 0), jstr!(env, recipe, 0), jstr!(env, dimension, 0), jstr!(env, items, 0), jstr!(env, player, 0));
    let items: Vec<String> = items.split('\n').map(str::to_string).collect();
    let pos = YogBlockPos { x, y, z };
    let mut made = false;
    guard("machine_craft", || {
        let Some(found) = machine_recipe(&def, &d, pos, &items, Some(&recipe)) else { return };
        let h = handlers();
        let Some(m) = h.inventories.iter().find(|i| i.id == def).and_then(|i| i.machine.as_ref()) else { return };
        let ev = machine_event(m, &def, &found, &d, pos, &player);
        let srv = srv_ptr();
        each(&h.machine_craft, |ud, f| unsafe { f(ud, srv, &ev, 1) });
        made = true;
    });
    made as jni::sys::jboolean
}

/// The Yog mods being loaded, by id (all of them, before the first registers).
static YOG_MOD_IDS: RwLock<std::collections::BTreeSet<String>> = RwLock::new(std::collections::BTreeSet::new());

/// Whether mod `id` is loaded: a Yog mod, else one in the host's list
/// (Fabric's, Forge's, NeoForge's).
fn mod_loaded(id: &str) -> bool {
    YOG_MOD_IDS.read().unwrap_or_else(|e| e.into_inner()).contains(id)
        || HOT_RELOADER.get().is_some_and(|r| r.all_active().iter().any(|(m, _)| m == id))
        // SAFETY: a host call with a string argument, on the calling thread.
        || unsafe { bridge!("isModLoaded", [JArg::S(YogStr::from_str(id))]) }.unwrap_or(false)
}

unsafe extern "C" fn api_is_mod_loaded(_ctx: *mut c_void, id: YogStr) -> bool {
    unsafe { mod_loaded(id.as_str()) }
}

unsafe extern "C" fn api_java_call(_ctx: *mut c_void, expr: YogStr, args: YogStr) -> YogOwnedStr {
    unsafe {
        let args = match yog_data::text::parse_value(args.as_str()) {
            Ok(v) => v,
            Err(e) => {
                yog_logging::warn!("java_call {}: its arguments aren't Yog Data: {}", expr.as_str(), e.message);
                return YogOwnedStr::NONE;
            }
        };
        let bytes = yog_data::binary::encode(&args);
        let out: Option<Vec<u8>> = host!("dev/yog/data/YogJava", "call", [JArg::S(expr), JArg::Bytes(&bytes)]);
        match out.map(|b| yog_data::binary::decode(&b)) {
            Some(Ok(v)) => YogOwnedStr::from_string(yog_data::text::to_string(&v)),
            _ => YogOwnedStr::NONE,
        }
    }
}

/// Strip TSV separators so one mod can't corrupt the line format.
fn tsv_clean(v: &str) -> String {
    v.replace(['\t', '\n', '\r'], " ")
}

unsafe extern "C" fn api_ui_open(_ctx: *mut c_void, ui_id: YogStr, modal: bool, pause: bool) {
    unsafe {
        if ui_id.as_str().is_empty() {
            return;
        }
        // Java side schedules onto the render thread; no-op on dedicated servers.
        let _: Option<()> = bridge!("openUI", [S(ui_id), Z(modal), Z(pause)]);
    }
}

unsafe extern "C" fn api_ui_close(_ctx: *mut c_void, ui_id: YogStr) {
    unsafe {
        // Java side defers onto the render thread (after the current event) and
        // checks the open screen is that Yog UI; no-op on dedicated servers.
        let _: Option<()> = bridge!("closeUI", [S(ui_id)]);
    }
}

unsafe extern "C" fn api_ui_consume_event(_ctx: *mut c_void) {
    ui_jni::consume_event();
}

unsafe extern "C" fn api_translate(_ctx: *mut c_void, key: YogStr) -> YogOwnedStr {
    unsafe { translate(key.as_str()).map_or(YogOwnedStr::NONE, YogOwnedStr::from_string) }
}

/// The game's text for translation `key` in the current language — `None`
/// when no lang file has it (or there's no JVM).
fn translate(key: &str) -> Option<String> {
    if key.is_empty() {
        return None;
    }
    unsafe { bridge!("translate", [S(YogStr::from_str(key))]) }
}

unsafe extern "C" fn api_mods_list(_ctx: *mut c_void) -> YogOwnedStr {
    unsafe {
        let mut lines: Vec<String> = Vec::new();
        let infos = MOD_INFOS.lock().expect("mod infos lock poisoned").clone();
        for m in &infos {
            // A `.yres` name/description shows in the player's language.
            let localized = |text: &str, key: &str| {
                translate(key).map_or_else(|| text.to_string(), |t| tsv_clean(&t))
            };
            lines.push(format!(
                "yog\t{}\t{}\t{}\t{}\t{}",
                m[0],
                localized(&m[1], &m[5]),
                m[2],
                m[3],
                localized(&m[4], &m[6])
            ));
        }
        // Loader-side (Java) mods, if the host provides them.
        let platform: Option<String> = bridge!("listPlatformMods", []);
        for line in platform.iter().flat_map(|s| s.lines()) {
            if !line.trim().is_empty() {
                lines.push(format!("platform\t{line}"));
            }
        }
        YogOwnedStr::from_string(lines.join("\n"))
    }
}

unsafe extern "C" fn api_register_menu_entry(ctx: *mut c_void, label: YogStr, ui_id: YogStr) {
    unsafe {
        let mut handlers = handlers_mut(ctx);
        let l = label.as_str().to_owned();
        let u = ui_id.as_str().to_owned();
        handlers.menu_entries.push((l, u));
    }
}

// ── World queries & effects (ABI minor 1) ────────────────────────────────────
//
// Each of these forwards to a `NativeBridge` static of the same shape; the JNI
// signature is derived from the argument list so the two cannot drift apart.

fn vec_args(v: YogVec3) -> [JArg<'static>; 3] {
    [JArg::D(v.x), JArg::D(v.y), JArg::D(v.z)]
}

fn pos_args(p: YogBlockPos) -> [JArg<'static>; 3] {
    [JArg::I(p.x), JArg::I(p.y), JArg::I(p.z)]
}

unsafe extern "C" fn srv_entity_info(_ctx: *mut c_void, uuid: YogStr) -> YogOwnedStr {
    unsafe { owned(bridge!("entityInfo", [JArg::S(uuid)])) }
}

unsafe extern "C" fn srv_entities_in_box(
    _ctx: *mut c_void,
    dim: YogStr,
    min: YogVec3,
    max: YogVec3,
) -> YogOwnedStr {
    unsafe {
        let [a, b, c] = vec_args(min);
        let [d, e, f] = vec_args(max);
        owned(bridge!("entitiesInBox", [JArg::S(dim), a, b, c, d, e, f]))
    }
}

unsafe extern "C" fn srv_raycast_block(
    _ctx: *mut c_void,
    dim: YogStr,
    from: YogVec3,
    to: YogVec3,
) -> YogOwnedStr {
    unsafe {
        let [a, b, c] = vec_args(from);
        let [d, e, f] = vec_args(to);
        owned(bridge!("raycastBlock", [JArg::S(dim), a, b, c, d, e, f]))
    }
}

unsafe extern "C" fn srv_raycast_entity(
    _ctx: *mut c_void,
    dim: YogStr,
    from: YogVec3,
    to: YogVec3,
    exclude: YogStr,
) -> YogOwnedStr {
    unsafe {
        let [a, b, c] = vec_args(from);
        let [d, e, f] = vec_args(to);
        owned(bridge!(
            "raycastEntity",
            [JArg::S(dim), a, b, c, d, e, f, JArg::S(exclude)]
        ))
    }
}

unsafe extern "C" fn srv_block_info(
    _ctx: *mut c_void,
    dim: YogStr,
    pos: YogBlockPos,
) -> YogOwnedStr {
    unsafe {
        let [x, y, z] = pos_args(pos);
        owned(bridge!("blockInfo", [JArg::S(dim), x, y, z]))
    }
}

unsafe extern "C" fn srv_break_block(
    _ctx: *mut c_void,
    dim: YogStr,
    pos: YogBlockPos,
    drop: bool,
    breaker: YogStr,
) -> bool {
    unsafe {
        let [x, y, z] = pos_args(pos);
        bridge!(
            "breakBlock",
            [JArg::S(dim), x, y, z, JArg::Z(drop), JArg::S(breaker)]
        )
        .unwrap_or(false)
    }
}

unsafe extern "C" fn srv_place_block_from_slot(
    _ctx: *mut c_void,
    player: YogStr,
    slot: u32,
    dim: YogStr,
    pos: YogBlockPos,
) -> bool {
    unsafe {
        let [x, y, z] = pos_args(pos);
        bridge!(
            "placeBlockFromSlot",
            [JArg::S(player), JArg::I(slot as i32), JArg::S(dim), x, y, z]
        )
        .unwrap_or(false)
    }
}

unsafe extern "C" fn srv_bonemeal(_ctx: *mut c_void, dim: YogStr, pos: YogBlockPos) -> bool {
    unsafe {
        let [x, y, z] = pos_args(pos);
        bridge!("bonemeal", [JArg::S(dim), x, y, z]).unwrap_or(false)
    }
}

unsafe extern "C" fn srv_explode(
    _ctx: *mut c_void,
    dim: YogStr,
    pos: YogVec3,
    power: f32,
    fire: bool,
    source: YogStr,
) -> bool {
    unsafe {
        let [x, y, z] = vec_args(pos);
        bridge!(
            "explode",
            [
                JArg::S(dim),
                x,
                y,
                z,
                JArg::F(power),
                JArg::Z(fire),
                JArg::S(source),
            ]
        )
        .unwrap_or(false)
    }
}

unsafe extern "C" fn srv_tellraw(_ctx: *mut c_void, player: YogStr, json: YogStr) -> bool {
    unsafe { bridge!("tellraw", [JArg::S(player), JArg::S(json)]).unwrap_or(false) }
}

unsafe extern "C" fn srv_entity_damage(
    _ctx: *mut c_void,
    uuid: YogStr,
    amount: f32,
    damage_type: YogStr,
    attacker: YogStr,
) -> bool {
    unsafe {
        bridge!(
            "entityDamage",
            [
                JArg::S(uuid),
                JArg::F(amount),
                JArg::S(damage_type),
                JArg::S(attacker),
            ]
        )
        .unwrap_or(false)
    }
}

unsafe extern "C" fn srv_entity_set_fire_ticks(
    _ctx: *mut c_void,
    uuid: YogStr,
    ticks: i32,
) -> bool {
    unsafe { bridge!("entitySetFireTicks", [JArg::S(uuid), JArg::I(ticks)]).unwrap_or(false) }
}

unsafe extern "C" fn srv_player_add_xp(_ctx: *mut c_void, player: YogStr, amount: i32) -> bool {
    unsafe { bridge!("playerAddXp", [JArg::S(player), JArg::I(amount)]).unwrap_or(false) }
}

unsafe extern "C" fn srv_player_set_may_fly(
    _ctx: *mut c_void,
    player: YogStr,
    may_fly: bool,
) -> bool {
    unsafe { bridge!("playerSetMayFly", [JArg::S(player), JArg::Z(may_fly)]).unwrap_or(false) }
}

unsafe extern "C" fn srv_player_info(_ctx: *mut c_void, player: YogStr) -> YogOwnedStr {
    unsafe { owned(bridge!("playerInfo", [JArg::S(player)])) }
}

unsafe extern "C" fn srv_spawn_item(
    _ctx: *mut c_void,
    dim: YogStr,
    pos: YogVec3,
    vel: YogVec3,
    item_id: YogStr,
    count: u32,
    snbt: YogStr,
) -> YogOwnedStr {
    unsafe {
        let [a, b, c] = vec_args(pos);
        let [d, e, f] = vec_args(vel);
        owned(bridge!(
            "spawnItem",
            [
                JArg::S(dim),
                a,
                b,
                c,
                d,
                e,
                f,
                JArg::S(item_id),
                JArg::I(count as i32),
                JArg::S(snbt),
            ]
        ))
    }
}

unsafe extern "C" fn srv_item_entity_stack(_ctx: *mut c_void, uuid: YogStr) -> YogOwnedStr {
    unsafe { owned(bridge!("itemEntityStack", [JArg::S(uuid)])) }
}

unsafe extern "C" fn srv_actionbar_json(_ctx: *mut c_void, player: YogStr, json: YogStr) -> bool {
    unsafe { bridge!("actionbarJson", [JArg::S(player), JArg::S(json)]).unwrap_or(false) }
}

unsafe extern "C" fn srv_use_item_on_block(
    _ctx: *mut c_void,
    dim: YogStr,
    pos: YogBlockPos,
    face: u8,
    item_id: YogStr,
    player: YogStr,
) -> bool {
    unsafe {
        let [x, y, z] = pos_args(pos);
        bridge!(
            "useItemOnBlock",
            [
                JArg::S(dim),
                x,
                y,
                z,
                JArg::I(face as i32),
                JArg::S(item_id),
                JArg::S(player),
            ]
        )
        .unwrap_or(false)
    }
}

unsafe extern "C" fn srv_pour_fluid(
    _ctx: *mut c_void,
    dim: YogStr,
    pos: YogBlockPos,
    bucket_item_id: YogStr,
) -> bool {
    unsafe {
        let [x, y, z] = pos_args(pos);
        bridge!(
            "pourFluid",
            [JArg::S(dim), x, y, z, JArg::S(bucket_item_id)]
        )
        .unwrap_or(false)
    }
}

unsafe extern "C" fn srv_has_entity_tag(
    _ctx: *mut c_void,
    entity_type: YogStr,
    tag: YogStr,
) -> bool {
    unsafe { bridge!("hasEntityTag", [JArg::S(entity_type), JArg::S(tag)]).unwrap_or(false) }
}

unsafe extern "C" fn srv_item_block(_ctx: *mut c_void, item_id: YogStr) -> YogOwnedStr {
    unsafe { owned(bridge!("itemBlock", [JArg::S(item_id)])) }
}

unsafe extern "C" fn srv_place_feature(
    _ctx: *mut c_void,
    dim: YogStr,
    pos: YogBlockPos,
    feature_id: YogStr,
) -> bool {
    unsafe {
        let [x, y, z] = pos_args(pos);
        bridge!("placeFeature", [JArg::S(dim), x, y, z, JArg::S(feature_id)]).unwrap_or(false)
    }
}

unsafe extern "C" fn srv_item_entity_set_stack(
    _ctx: *mut c_void,
    uuid: YogStr,
    item_id: YogStr,
    count: u32,
    snbt: YogStr,
) -> bool {
    unsafe {
        bridge!(
            "itemEntitySetStack",
            [
                JArg::S(uuid),
                JArg::S(item_id),
                JArg::I(count as i32),
                JArg::S(snbt),
            ]
        )
        .unwrap_or(false)
    }
}

// ── Table constructors ────────────────────────────────────────────────────────

fn build_server_table() -> YogServer {
    YogServer {
        ctx: std::ptr::null_mut(),
        abi_version: ABI_VERSION,
        size: std::mem::size_of::<YogServer>() as u32,
        free_str: yog_free_str,
        broadcast: srv_broadcast,
        get_block: srv_get_block,
        set_block: srv_set_block,
        world_time: srv_world_time,
        set_time: srv_set_time,
        is_raining: srv_is_raining,
        set_weather: srv_set_weather,
        give_item: srv_give_item,
        player_teleport: srv_player_teleport,
        send_to_player: srv_send_to_player,
        send_to_server: srv_send_to_server,
        kick_player: srv_kick_player,
        set_gamemode: srv_set_gamemode,
        send_title: srv_send_title,
        send_actionbar: srv_send_actionbar,
        play_sound: srv_play_sound,
        play_sound_player: srv_play_sound_player,
        entity_teleport: srv_entity_teleport,
        entity_position: srv_entity_position,
        entity_rotation: srv_entity_rotation,
        entity_health: srv_entity_health,
        entity_set_health: srv_entity_set_health,
        entity_kill: srv_entity_kill,
        spawn_entity: srv_spawn_entity,
        entity_add_effect: srv_entity_add_effect,
        entity_remove_effect: srv_entity_remove_effect,
        entity_clear_effects: srv_entity_clear_effects,
        entity_velocity: srv_entity_velocity,
        entity_set_velocity: srv_entity_set_velocity,
        entity_add_velocity: srv_entity_add_velocity,
        has_item_tag: srv_has_item_tag,
        has_block_tag: srv_has_block_tag,
        drop_loot: srv_drop_loot,
        scoreboard_get: srv_scoreboard_get,
        scoreboard_set: srv_scoreboard_set,
        scoreboard_add: srv_scoreboard_add,
        bossbar_create: srv_bossbar_create,
        bossbar_remove: srv_bossbar_remove,
        bossbar_set_title: srv_bossbar_set_title,
        bossbar_set_progress: srv_bossbar_set_progress,
        bossbar_set_color: srv_bossbar_set_color,
        bossbar_add_player: srv_bossbar_add_player,
        bossbar_remove_player: srv_bossbar_remove_player,
        bossbar_set_visible: srv_bossbar_set_visible,
        game_dir: srv_game_dir,
        get_block_nbt: srv_get_block_nbt,
        set_block_nbt: srv_set_block_nbt,
        get_inventory_slot: srv_get_inventory_slot,
        set_inventory_slot: srv_set_inventory_slot,
        player_inventory: srv_player_inventory,
        player_set_slot: srv_player_set_slot,
        player_teleport_dim: srv_player_teleport_dim,
        entity_teleport_dim: srv_entity_teleport_dim,
        online_players: srv_online_players,
        world_entity_count: srv_world_entity_count,
        entity_get_nbt: srv_entity_get_nbt,
        entity_set_nbt: srv_entity_set_nbt,
        spawn_particles: srv_spawn_particles,
        entity_attribute_get: srv_entity_attribute_get,
        entity_attribute_set: srv_entity_attribute_set,
        get_held_item_nbt: srv_get_held_item_nbt,
        set_held_item_nbt: srv_set_held_item_nbt,
        get_offhand_item_nbt: srv_get_offhand_item_nbt,
        set_offhand_item_nbt: srv_set_offhand_item_nbt,
        get_slot_item: srv_get_slot_item,
        set_slot_item: srv_set_slot_item,
        entity_info: srv_entity_info,
        entities_in_box: srv_entities_in_box,
        raycast_block: srv_raycast_block,
        raycast_entity: srv_raycast_entity,
        block_info: srv_block_info,
        break_block: srv_break_block,
        place_block_from_slot: srv_place_block_from_slot,
        bonemeal: srv_bonemeal,
        explode: srv_explode,
        tellraw: srv_tellraw,
        entity_damage: srv_entity_damage,
        entity_set_fire_ticks: srv_entity_set_fire_ticks,
        player_add_xp: srv_player_add_xp,
        player_set_may_fly: srv_player_set_may_fly,
        player_info: srv_player_info,
        spawn_item: srv_spawn_item,
        item_entity_stack: srv_item_entity_stack,
        item_entity_set_stack: srv_item_entity_set_stack,
        actionbar_json: srv_actionbar_json,
        use_item_on_block: srv_use_item_on_block,
        pour_fluid: srv_pour_fluid,
        has_entity_tag: srv_has_entity_tag,
        item_block: srv_item_block,
        place_feature: srv_place_feature,
        set_item_cooldown: srv_set_item_cooldown,
        entity_vehicle: srv_entity_vehicle,
        entity_passengers: srv_entity_passengers,
        entity_mount: srv_entity_mount,
        entity_dismount: srv_entity_dismount,
        redstone_power: srv_redstone_power,
        entity_set_dimensions: srv_entity_set_dimensions,
        entity_data_set: entity_data::srv_entity_data_set,
        entity_data_get: entity_data::srv_entity_data_get,
        entity_set_fall_distance: srv_entity_set_fall_distance,
        redstone_signal: srv_redstone_signal,
        set_redstone_output: srv_set_redstone_output,
        get_block_state: srv_get_block_state,
        set_block_state: srv_set_block_state,
        world_is_thundering: srv_world_is_thundering,
        world_set_thunder: srv_world_set_thunder,
        fill_region: srv_fill_region,
        scan_region: srv_scan_region,
    }
}

// ── ABI minor 3 ──────────────────────────────────────────────────────────────

unsafe extern "C" fn srv_set_item_cooldown(_ctx: *mut c_void, player: YogStr, item_id: YogStr, ticks: i32) -> bool {
    unsafe { bridge!("setItemCooldown", [S(player), S(item_id), I(ticks)]).unwrap_or(false) }
}

unsafe extern "C" fn srv_entity_vehicle(_ctx: *mut c_void, uuid: YogStr) -> YogOwnedStr {
    unsafe { owned(bridge!("entityVehicle", [S(uuid)])) }
}

unsafe extern "C" fn srv_entity_passengers(_ctx: *mut c_void, uuid: YogStr) -> YogOwnedStr {
    unsafe { owned(bridge!("entityPassengers", [S(uuid)])) }
}

unsafe extern "C" fn srv_entity_mount(_ctx: *mut c_void, rider: YogStr, vehicle: YogStr, force: bool) -> bool {
    unsafe { bridge!("entityMount", [S(rider), S(vehicle), Z(force)]).unwrap_or(false) }
}

unsafe extern "C" fn srv_entity_dismount(_ctx: *mut c_void, uuid: YogStr) -> bool {
    unsafe { bridge!("entityDismount", [S(uuid)]).unwrap_or(false) }
}

/// The server's hitbox now, the clients' through entity data.
unsafe extern "C" fn srv_entity_set_dimensions(_ctx: *mut c_void, uuid: YogStr, w: f32, h: f32, eye: f32) -> bool {
    let found: bool = unsafe { bridge!("entitySetDimensions", [S(uuid), F(w), F(h), F(eye)]).unwrap_or(false) };
    if found {
        let value = (w >= 0.0).then(|| entity_data::encode_dims(w, h, eye));
        entity_data::set(unsafe { uuid.as_str() }, entity_data::DIMENSIONS, value.as_deref(), false);
    }
    found
}

unsafe extern "C" fn srv_entity_set_fall_distance(_ctx: *mut c_void, uuid: YogStr, distance: f32) -> bool {
    unsafe { bridge!("entitySetFallDistance", [S(uuid), F(distance)]).unwrap_or(false) }
}

unsafe extern "C" fn srv_redstone_signal(_ctx: *mut c_void, dim: YogStr, pos: YogBlockPos) -> i32 {
    unsafe { bridge!("redstoneSignal", [S(dim), I(pos.x), I(pos.y), I(pos.z)]).unwrap_or(-1) }
}

unsafe extern "C" fn srv_redstone_power(_ctx: *mut c_void, dim: YogStr, pos: YogBlockPos) -> i32 {
    unsafe { bridge!("redstonePower", [S(dim), I(pos.x), I(pos.y), I(pos.z)]).unwrap_or(-1) }
}

unsafe extern "C" fn srv_set_redstone_output(_ctx: *mut c_void, dim: YogStr, pos: YogBlockPos, level: i32) -> bool {
    unsafe { bridge!("setRedstoneOutput", [S(dim), I(pos.x), I(pos.y), I(pos.z), I(level)]).unwrap_or(false) }
}

unsafe extern "C" fn srv_get_block_state(_ctx: *mut c_void, dim: YogStr, pos: YogBlockPos) -> YogOwnedStr {
    unsafe { owned(bridge!("getBlockState", [S(dim), I(pos.x), I(pos.y), I(pos.z)])) }
}

unsafe extern "C" fn srv_set_block_state(_ctx: *mut c_void, dim: YogStr, pos: YogBlockPos, state: YogStr) -> bool {
    unsafe { bridge!("setBlockState", [S(dim), I(pos.x), I(pos.y), I(pos.z), S(state)]).unwrap_or(false) }
}

unsafe extern "C" fn srv_world_is_thundering(_ctx: *mut c_void, dim: YogStr) -> bool {
    unsafe { bridge!("worldIsThundering", [S(dim)]).unwrap_or(false) }
}

unsafe extern "C" fn srv_world_set_thunder(_ctx: *mut c_void, dim: YogStr, thundering: bool, duration_ticks: i32) -> bool {
    unsafe { bridge!("worldSetThunder", [S(dim), Z(thundering), I(duration_ticks)]).unwrap_or(false) }
}

unsafe extern "C" fn srv_fill_region(
    _ctx: *mut c_void,
    dim: YogStr,
    from: YogBlockPos,
    to: YogBlockPos,
    state: YogStr,
) -> i32 {
    unsafe {
        bridge!(
            "fillRegion",
            [S(dim), I(from.x), I(from.y), I(from.z), I(to.x), I(to.y), I(to.z), S(state)]
        )
        .unwrap_or(-1)
    }
}

unsafe extern "C" fn srv_scan_region(_ctx: *mut c_void, dim: YogStr, from: YogBlockPos, to: YogBlockPos) -> YogOwnedStr {
    unsafe { owned(bridge!("scanRegion", [S(dim), I(from.x), I(from.y), I(from.z), I(to.x), I(to.y), I(to.z)])) }
}

// ── Inter-mod communication ──────────────────────────────────────────────────

unsafe extern "C" fn interop_export_impl(
    _ctx: *mut c_void,
    mod_id: YogStr,
    symbol: YogStr,
    ptr: *const c_void,
) {
    unsafe {
        let mod_id = mod_id.as_str();
        let sym = symbol.as_str();
        let registry = INTEROP_SYMBOLS.get_or_init(|| Mutex::new(HashMap::new()));
        registry
            .lock()
            .expect("interop symbols lock poisoned")
            .entry(mod_id.to_string())
            .or_default()
            .insert(sym.to_string(), ptr as usize);
    }
}

unsafe extern "C" fn interop_import_impl(
    _ctx: *mut c_void,
    mod_id: YogStr,
    symbol: YogStr,
) -> *const c_void {
    unsafe {
        let mod_id = mod_id.as_str();
        let sym = symbol.as_str();
        let registry = INTEROP_SYMBOLS.get_or_init(|| Mutex::new(HashMap::new()));
        registry
            .lock()
            .expect("interop symbols lock poisoned")
            .get(mod_id)
            .and_then(|m: &HashMap<String, usize>| m.get(sym))
            .map(|&ptr| ptr as *const c_void)
            .unwrap_or(std::ptr::null())
    }
}

fn build_api_table(ctx: *const RwLock<RuntimeHandlers>, server: *const YogServer) -> YogApi {
    YogApi {
        abi_version: ABI_VERSION,
        size: std::mem::size_of::<YogApi>() as u32,
        ctx: ctx as *mut c_void,
        server,
        on_block_break: api_on_block_break,
        on_chat: api_on_chat,
        on_player_join: api_on_player_join,
        on_player_leave: api_on_player_leave,
        on_use_item: api_on_use_item,
        on_use_block: api_on_use_block,
        on_attack_entity: api_on_attack_entity,
        on_entity_damage: api_on_entity_damage,
        on_entity_death: api_on_entity_death,
        on_entity_spawn: api_on_entity_spawn,
        on_player_place_block: api_on_player_place_block,
        on_player_death: api_on_player_death,
        on_player_respawn: api_on_player_respawn,
        on_advancement: api_on_advancement,
        on_entity_interact: api_on_entity_interact,
        on_item_craft: api_on_item_craft,
        on_explosion: api_on_explosion,
        on_item_pickup: api_on_item_pickup,
        on_item_insert: api_on_item_insert,
        on_player_move: api_on_player_move,
        on_container_open: api_on_container_open,
        on_container_close: api_on_container_close,
        on_projectile_hit: api_on_projectile_hit,
        on_server_tick: api_on_server_tick,
        on_server_started: api_on_server_started,
        on_server_stopping: api_on_server_stopping,
        on_packet: api_on_packet,
        on_client_packet: api_on_client_packet,
        register_command: api_register_command,
        register_typed_command: api_register_typed_command,
        register_content: api_register_content,
        schedule_once: api_schedule_once,
        schedule_repeating: api_schedule_repeating,
        on_client_tick: api_on_client_tick,
        on_hud_render: api_on_hud_render,
        on_key_press: api_on_key_press,
        on_screen_open: api_on_screen_open,
        on_screen_close: api_on_screen_close,
        on_world_render: api_on_world_render,
        register_ui: api_register_ui,
        on_ui_render: api_on_ui_render,
        register_menu_entry: api_register_menu_entry,
        mods_list: api_mods_list,
        free_str: yog_free_str,
        ui_open: api_ui_open,
        interop_export: interop_export_impl,
        interop_import: interop_import_impl,
        register_chunk_generator: api_register_chunk_generator,
        ui_close: api_ui_close,
        ui_consume_event: api_ui_consume_event,
        translate: api_translate,
        ui_open_flags: client_api::api_ui_open_flags,
        client_play_sound: client_api::api_client_play_sound,
        client_stop_sound: client_api::api_client_stop_sound,
        client_held_item: client_api::api_client_held_item,
        client_set_block_tint: client_api::api_client_set_block_tint,
        client_key_down: client_api::api_client_key_down,
        on_book_page: api_on_book_page,
        on_item_tooltip: api_on_item_tooltip,
        is_mod_loaded: api_is_mod_loaded,
        java_call: api_java_call,
        on_item_text: api_on_item_text,
        client_body_track: body::api_client_body_track,
        client_body_track_type: body::api_client_body_track_type,
        client_set_dimensions: body::api_client_set_dimensions,
        client_entity_data: entity_data::api_client_entity_data,
        on_body: body::api_on_body,
        on_camera: body::api_on_camera,
        on_travel: body::api_on_travel,
        on_machine_craft: api_on_machine_craft,
        on_entity_tick: api_on_entity_tick,
    }
}

// ── Mod loading ───────────────────────────────────────────────────────────────

fn platform_tag() -> String {
    format!("{}-{}", std::env::consts::OS, std::env::consts::ARCH)
}

type AbiVersionFn = unsafe extern "C" fn() -> u32;
/// Second arg is a null-terminated C string: the mod's `id` from its manifest.
type RegisterFn = unsafe extern "C" fn(*const YogApi, *const std::os::raw::c_char);

fn load_mods(dir: &Path, api: &YogApi) {
    let entries = match std::fs::read_dir(dir) {
        Ok(e) => e,
        Err(_) => {
            yog_logging::info!("no mods directory at {} — none loaded", dir.display());
            return;
        }
    };

    // First pass: collect (mod_id, dependencies, archive_path)
    struct ModMeta {
        id: String,
        deps: Vec<String>,
        yog_path: PathBuf,
    }
    let mut metas: Vec<ModMeta> = Vec::new();
    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("yog") {
            continue;
        }
        let info = read_mod_info(&path);
        let deps = read_mod_deps(&path);
        metas.push(ModMeta {
            id: info[0].clone(),
            deps,
            yog_path: path,
        });
    }

    // Topological sort by dependencies (Kahn's algorithm)
    let ids: Vec<String> = metas.iter().map(|m| m.id.clone()).collect();
    // Known before any registers: a mod's optional integrations with a Yog
    // mod loaded after it still see it.
    YOG_MOD_IDS.write().unwrap_or_else(|e| e.into_inner()).extend(ids.iter().cloned());
    let deps_list: Vec<Vec<String>> = metas.iter().map(|m| m.deps.clone()).collect();
    let sorted = match topo_sort_mods(&ids, &deps_list) {
        Some(v) => v,
        None => {
            yog_logging::error!("circular dependency detected — loading in file order");
            (0..metas.len()).collect()
        }
    };

    let mut loaded: Vec<LoadedModule> = Vec::new();
    for idx in sorted {
        let meta = &metas[idx];
        let lib_path = match extract_yog(&meta.yog_path) {
            Some(p) => p,
            None => {
                yog_logging::error!(
                    "no native for {} in {}",
                    platform_tag(),
                    meta.yog_path.display()
                );
                continue;
            }
        };
        if let Some(module) = load_mod_lib(&lib_path, api, &meta.id) {
            loaded.push(module);
            MOD_INFOS
                .lock()
                .expect("mod infos lock poisoned")
                .push(read_mod_info(&meta.yog_path));
        }
    }
    yog_logging::info!("loaded {} mod(s) from {}", loaded.len(), dir.display());

    // Final import resolution pass — after all mods loaded, each mod's pending
    // imports can be resolved against the now-complete interop table.
    resolve_all_imports(api, &loaded);
}

/// After all mods loaded, call `__yog_resolve_imports_final` on each library
/// to resolve any imports that weren't satisfied during initial registration.
fn resolve_all_imports(api: &YogApi, loaded: &[LoadedModule]) {
    for module in loaded {
        module.with_library(|lib| unsafe {
            type ResolveFn = unsafe extern "C" fn(*const YogApi);
            if let Ok(resolve) = lib.get::<ResolveFn>(b"__yog_resolve_imports_final") {
                resolve(api as *const YogApi);
            }
        });
    }
}

/// Kahn's algorithm — returns indices in dependency order, or None on cycle.
fn topo_sort_mods(ids: &[String], deps_list: &[Vec<String>]) -> Option<Vec<usize>> {
    let n = ids.len();
    let name_to_idx: HashMap<&str, usize> = ids
        .iter()
        .enumerate()
        .map(|(i, id)| (id.as_str(), i))
        .collect();
    let mut in_degree = vec![0u32; n];
    let mut adj: Vec<Vec<usize>> = vec![Vec::new(); n];
    for (i, deps) in deps_list.iter().enumerate() {
        for dep in deps {
            if let Some(&j) = name_to_idx.get(dep.as_str()) {
                adj[j].push(i);
                in_degree[i] += 1;
            }
        }
    }
    let mut queue: Vec<usize> = (0..n).filter(|&i| in_degree[i] == 0).collect();
    let mut order = Vec::with_capacity(n);
    while let Some(u) = queue.pop() {
        order.push(u);
        for &v in &adj[u] {
            in_degree[v] -= 1;
            if in_degree[v] == 0 {
                queue.push(v);
            }
        }
    }
    if order.len() == n { Some(order) } else { None }
}

/// Loads `path` as a fresh generation of `mod_id` via the shared
/// `HotReloader`, validates its ABI, and calls its `yog_mod_register` —
/// tagging every handler it registers with the returned `LoadedModule` (see
/// `current_registration()`). On any failure the module (if it got as far
/// as being tracked by the `HotReloader`) is retired immediately: nothing
/// can be executing inside a library that never successfully registered,
/// so this always hard-unloads right away rather than lingering.
fn load_mod_lib(path: &Path, api: &YogApi, mod_id: &str) -> Option<LoadedModule> {
    let module = match unsafe { hot_reloader().load_initial(mod_id, path) } {
        Ok(m) => m,
        Err(e) => {
            yog_logging::error!("failed to load {}: {}", path.display(), e);
            return None;
        }
    };

    let abi = module.with_library(|lib| unsafe {
        let abi: Symbol<AbiVersionFn> = lib.get(b"yog_abi_version").ok()?;
        Some(abi())
    });
    match abi.flatten() {
        Some(v) if v == ABI_VERSION => {}
        Some(v) => {
            yog_logging::error!(
                "{}: ABI {} incompatible with runtime ABI {}",
                path.display(),
                v,
                ABI_VERSION
            );
            module.retire(|| {});
            return None;
        }
        None => {
            yog_logging::error!("{} is not a Yog mod (no yog_abi_version)", path.display());
            module.retire(|| {});
            return None;
        }
    }

    if let Err(e) = invoke_mod_register(&module, api, mod_id) {
        yog_logging::error!("{}: {}", path.display(), e);
        module.retire(|| {});
        return None;
    }

    Some(module)
}

/// Calls a loaded module's `yog_mod_register`, tagging everything it
/// registers with `module` via `CURRENT_REGISTRATION` for the duration of
/// the call — used for both a mod's initial load and a later hot-reload.
fn invoke_mod_register(module: &LoadedModule, api: &YogApi, mod_id: &str) -> Result<(), String> {
    module
        .with_library(|lib| unsafe {
            let register: Symbol<RegisterFn> = lib
                .get(b"yog_mod_register")
                .map_err(|_| "missing yog_mod_register".to_string())?;
            let mod_id_c = std::ffi::CString::new(mod_id).unwrap();
            *CURRENT_REGISTRATION.lock().unwrap() = Some(module.clone());
            register(api as *const YogApi, mod_id_c.as_ptr());
            *CURRENT_REGISTRATION.lock().unwrap() = None;
            Ok(())
        })
        .unwrap_or_else(|| Err("module was unloaded before registration could run".to_string()))
}

/// Metadata for a mod file: [id, name, version, authors, description,
/// name_key, description_key].
/// For .yog archives this comes from the bundled yog.toml; bare native libs
/// fall back to the file stem.
fn read_mod_info(path: &Path) -> [String; 7] {
    let stem = path
        .file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_default();
    let mut info = [
        stem.clone(),
        stem,
        String::new(),
        String::new(),
        String::new(),
        String::new(),
        String::new(),
    ];
    if path.extension().and_then(|e| e.to_str()) != Some("yog") {
        return info;
    }
    let Some(text) = (|| {
        let file = std::fs::File::open(path).ok()?;
        let mut archive = zip::ZipArchive::new(file).ok()?;
        let mut entry = archive.by_name("yog.toml").ok()?;
        let mut text = String::new();
        std::io::Read::read_to_string(&mut entry, &mut text).ok()?;
        Some(text)
    })() else {
        return info;
    };
    // Minimal parse — enough for id/name/version/description/authors without
    // pulling in a TOML dependency. Source manifests keep these under [mod];
    // the packaged manifest (written by yog-cli) has them at top level, so
    // keys before any section header count too.
    let mut in_mod = true;
    for line in text.lines() {
        let line = line.trim();
        if line.starts_with('[') {
            in_mod = line == "[mod]";
            continue;
        }
        if !in_mod {
            continue;
        }
        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        let key = key.trim();
        let value = value.trim();
        let unquote = |v: &str| v.trim().trim_matches('"').to_string();
        match key {
            "id" => info[0] = tsv_clean(&unquote(value)),
            "name" => info[1] = tsv_clean(&unquote(value)),
            "version" => info[2] = tsv_clean(&unquote(value)),
            "description" => info[4] = tsv_clean(&unquote(value)),
            "name_key" => info[5] = unquote(value),
            "description_key" => info[6] = unquote(value),
            "authors" => {
                let list = value.trim_start_matches('[').trim_end_matches(']');
                let authors: Vec<String> = list
                    .split(',')
                    .map(unquote)
                    .filter(|a| !a.is_empty())
                    .collect();
                info[3] = tsv_clean(&authors.join(", "));
            }
            _ => {}
        }
    }
    info
}

/// Parse `[dependencies]` section from yog.toml inside a .yog archive.
fn read_mod_deps(path: &Path) -> Vec<String> {
    if path.extension().and_then(|e| e.to_str()) != Some("yog") {
        return Vec::new();
    }
    let text = (|| {
        let file = std::fs::File::open(path).ok()?;
        let mut archive = zip::ZipArchive::new(file).ok()?;
        let mut entry = archive.by_name("yog.toml").ok()?;
        let mut text = String::new();
        std::io::Read::read_to_string(&mut entry, &mut text).ok()?;
        Some(text)
    })();
    let Some(text) = text else { return Vec::new() };
    let mut deps = Vec::new();
    let mut in_deps = false;
    for line in text.lines() {
        let line = line.trim();
        if line.starts_with('[') {
            in_deps = line == "[dependencies]";
            continue;
        }
        if !in_deps {
            continue;
        }
        // `mod_id = "1.0"` or `mod_id = "*"` — we only care about the key (mod_id)
        if let Some((key, _)) = line.split_once('=') {
            deps.push(key.trim().to_string());
        }
    }
    deps
}

fn extract_yog(path: &Path) -> Option<PathBuf> {
    let file = std::fs::File::open(path).ok()?;
    let mut archive = zip::ZipArchive::new(file).ok()?;
    let prefix = format!("natives/{}/", platform_tag());
    let mut entry_name = None;
    for i in 0..archive.len() {
        let f = archive.by_index(i).ok()?;
        let name = f.name().ok()?;
        if name.starts_with(&prefix) && !name.ends_with('/') {
            entry_name = Some(name.to_string());
            break;
        }
    }
    let entry_name = entry_name?;
    let ext = Path::new(&entry_name)
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("bin");
    let stem = path.file_stem()?.to_string_lossy().into_owned();
    // A fresh name per extraction: a hot reload must never truncate the
    // library the game still has mapped, and `dlopen` of an already-loaded
    // path would just hand back the old module.
    static EXTRACTIONS: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
    let n = EXTRACTIONS.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let out =
        std::env::temp_dir().join(format!("yog-{}-{}-{}.{}", stem, std::process::id(), n, ext));
    let mut entry = archive.by_name(&entry_name).ok()?;
    let mut out_file = std::fs::File::create(&out).ok()?;
    std::io::copy(&mut entry, &mut out_file).ok()?;
    Some(out)
}

// ── Dispatcher helpers ────────────────────────────────────────────────────────

fn srv_ptr() -> *const YogServer {
    SERVER.get().expect("yog: SERVER not initialised") as *const YogServer
}

// ── JNI entry points ──────────────────────────────────────────────────────────

#[unsafe(no_mangle)]
pub extern "system" fn Java_dev_yog_NativeBridge_nativeInit<'l>(
    mut env: JNIEnv<'l>,
    _class: JClass<'l>,
    mods_dir: JString<'l>,
) {
    if let Ok(vm) = env.get_java_vm() {
        let _ = JAVA_VM.set(vm);
    }

    let dir = env
        .get_string(&mods_dir)
        .map(String::from)
        .unwrap_or_default();

    // Build YogServer and store in static (gets a stable address).
    let _ = SERVER.set(build_server_table());
    let server_ptr = SERVER.get().unwrap() as *const YogServer;

    // Publish HANDLERS *before* any mod registers — its address is then
    // `'static`-stable for the runtime's entire lifetime, unlike the old
    // design (a temporary heap `Box` moved into `HANDLERS` only after
    // `nativeInit` returned, silently invalidating the `ctx` pointer any
    // later re-registration — e.g. a hot-reload — would have needed).
    let handlers_lock = HANDLERS.get_or_init(|| RwLock::new(RuntimeHandlers::default()));
    let handlers_ptr = handlers_lock as *const RwLock<RuntimeHandlers>;

    let api: &'static YogApi = API_TABLE.get_or_init(|| build_api_table(handlers_ptr, server_ptr));

    let _ = HOT_RELOADER.get_or_init(|| HotReloader::new(RuntimeModuleRegistry));

    guard("mod loading", || {
        load_mods(Path::new(&dir), api);
    });

    control_socket::start_if_requested();

    yog_logging::info!("runtime initialised — the gate is open.");
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_dev_yog_NativeBridge_nativeHotReload<'l>(
    mut env: JNIEnv<'l>,
    _class: JClass<'l>,
    mod_id: JString<'l>,
    new_yog_path: JString<'l>,
) -> jni::sys::jboolean {
    let Ok(mod_id) = env.get_string(&mod_id).map(String::from) else {
        return 0;
    };
    let Ok(yog_path) = env.get_string(&new_yog_path).map(String::from) else {
        return 0;
    };
    let ok = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        trigger_hot_reload(&mod_id, Path::new(&yog_path))
    }))
    .unwrap_or(false);
    ok as jni::sys::jboolean
}

/// Rebuilds and re-registers `mod_id` from a freshly built `.yog` at
/// `new_yog_path`: extracts the current platform's native, loads it as a
/// new generation via the shared `HotReloader` (which retires the previous
/// generation per its hybrid soft/hard-unload policy — hard-unloaded right
/// away if nothing's mid-call, deferred otherwise), then calls its
/// `yog_mod_register` again so its handlers get tagged under the new
/// generation. How this actually gets triggered — a keybind, a file-watch,
/// an IDE-driven socket — is a separate, later concern; this only needs to
/// work correctly once called.
fn trigger_hot_reload(mod_id: &str, new_yog_path: &Path) -> bool {
    let Some(native_path) = extract_yog(new_yog_path) else {
        yog_logging::error!(
            "hot-reload {}: no native for {} in {}",
            mod_id,
            platform_tag(),
            new_yog_path.display()
        );
        return false;
    };

    let api = API_TABLE.get().expect("yog: nativeInit not called yet");

    let module = match unsafe { hot_reloader().reload(mod_id, &native_path) } {
        Ok(m) => m,
        Err(e) => {
            yog_logging::error!(
                "hot-reload {}: failed to load {}: {}",
                mod_id,
                native_path.display(),
                e
            );
            return false;
        }
    };

    let abi = module.with_library(|lib| unsafe {
        let abi: Symbol<AbiVersionFn> = lib.get(b"yog_abi_version").ok()?;
        Some(abi())
    });
    match abi.flatten() {
        Some(v) if v == ABI_VERSION => {}
        other => {
            yog_logging::error!(
                "hot-reload {}: ABI {:?} incompatible with runtime ABI {} — mod has no active handlers until a compatible reload succeeds",
                mod_id,
                other,
                ABI_VERSION
            );
            return false;
        }
    }

    if let Err(e) = invoke_mod_register(&module, api, mod_id) {
        yog_logging::error!("hot-reload {}: {}", mod_id, e);
        return false;
    }

    yog_logging::info!("hot-reloaded mod {}", mod_id);
    true
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_dev_yog_NativeBridge_nativeOnBlockBreak<'l>(
    mut env: JNIEnv<'l>,
    _class: JClass<'l>,
    player: JString<'l>,
    block: JString<'l>,
    x: jint,
    y: jint,
    z: jint,
    dimension: JString<'l>,
) {
    let (p, b, d) = (jstr!(env, player), jstr!(env, block), jstr!(env, dimension));
    let ev = yog_abi::YogBlockBreakEvent {
        player: YogStr::from_str(&p),
        block: YogStr::from_str(&b),
        pos: YogBlockPos { x, y, z },
        dimension: YogStr::from_str(&d),
    };
    let srv = srv_ptr();
    guard("on_block_break", || {
        each(&handlers().block_break, |ud, f| unsafe {
            f(ud, srv, &ev, 1)
        });
    });
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_dev_yog_NativeBridge_nativeOnChat<'l>(
    mut env: JNIEnv<'l>,
    _class: JClass<'l>,
    player: JString<'l>,
    message: JString<'l>,
    dimension: JString<'l>,
) {
    let (p, m, d) = (
        jstr!(env, player),
        jstr!(env, message),
        jstr!(env, dimension),
    );
    let ev = yog_abi::YogChatEvent {
        player: YogStr::from_str(&p),
        message: YogStr::from_str(&m),
        dimension: YogStr::from_str(&d),
    };
    let srv = srv_ptr();
    guard("on_chat", || {
        each(&handlers().chat, |ud, f| unsafe { f(ud, srv, &ev, 1) });
    });
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_dev_yog_NativeBridge_nativeOnPlayerJoin<'l>(
    mut env: JNIEnv<'l>,
    _class: JClass<'l>,
    player: JString<'l>,
    uuid: JString<'l>,
    dimension: JString<'l>,
) {
    let (p, u, d) = (jstr!(env, player), jstr!(env, uuid), jstr!(env, dimension));
    let ev = yog_abi::YogPlayerEvent {
        player: YogStr::from_str(&p),
        uuid: YogStr::from_str(&u),
        dimension: YogStr::from_str(&d),
    };
    let srv = srv_ptr();
    guard("on_player_join", || {
        each(&handlers().player_join, |ud, f| unsafe {
            f(ud, srv, &ev, 1)
        });
    });
    // Queue startup grants: giving items directly inside the join callback
    // fails for the host of a freshly created world (player not fully in the
    // world yet), so grants are processed a few server ticks later.
    let h = handlers();
    if !h.startup_grants.is_empty() {
        h.pending_grants
            .lock()
            .expect("pending_grants poisoned")
            .push((p, u, 0));
    }
}

/// The command tag a player carries once startup grant `id` was given. It's
/// saved with the player's own data, so it follows the player — not their
/// name or uuid (a dev client gets a new name each launch, while a
/// singleplayer world's owner stays the same player).
fn grant_tag(id: &str) -> String {
    format!("yog.grant.{id}")
}

/// Try to fulfil pending startup grants. Called each server tick; a grant is
/// only marked as given (tagged on the player) after `give_item` succeeds.
fn process_pending_grants() {
    let h = handlers();
    let mut pending = match h.pending_grants.try_lock() {
        Ok(g) => g,
        Err(_) => return,
    };
    if pending.is_empty() {
        return;
    }
    // Wait a couple of ticks after join before the first attempt.
    const FIRST_TRY_TICK: u32 = 2;
    // Give up after 30 seconds — the player probably left.
    const MAX_TICKS: u32 = 600;

    let mut still_pending = Vec::new();
    for (p, u, ticks) in pending.drain(..) {
        if ticks < FIRST_TRY_TICK {
            still_pending.push((p, u, ticks + 1));
            continue;
        }
        let player = YogStr::from_str(&p);
        let mut all_ok = true;
        for sg in &h.startup_grants {
            let tag = grant_tag(&sg.id);
            let tag = YogStr::from_str(&tag);
            if unsafe { bridge!("playerHasTag", [S(player), S(tag)]) }.unwrap_or(false) {
                continue;
            }
            let book = sg.book.as_ref().map(|_| "minecraft:written_book");
            let mut ok = true;
            for item_id in sg.items.iter().map(String::as_str).chain(book) {
                ok &= unsafe { srv_give_item(std::ptr::null_mut(), player, YogStr::from_str(item_id), 1) };
            }
            if ok && unsafe { bridge!("addPlayerTag", [S(player), S(tag)]) }.unwrap_or(false) {
                yog_logging::info!("startup grant {} given to {}", sg.id, p);
            } else {
                all_ok = false;
            }
        }
        if !all_ok && ticks < MAX_TICKS {
            still_pending.push((p, u, ticks + 1));
        }
    }
    *pending = still_pending;
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_dev_yog_NativeBridge_nativeOnPlayerLeave<'l>(
    mut env: JNIEnv<'l>,
    _class: JClass<'l>,
    player: JString<'l>,
    uuid: JString<'l>,
    dimension: JString<'l>,
) {
    let (p, u, d) = (jstr!(env, player), jstr!(env, uuid), jstr!(env, dimension));
    let ev = yog_abi::YogPlayerEvent {
        player: YogStr::from_str(&p),
        uuid: YogStr::from_str(&u),
        dimension: YogStr::from_str(&d),
    };
    let srv = srv_ptr();
    guard("on_player_leave", || {
        each(&handlers().player_leave, |ud, f| unsafe {
            f(ud, srv, &ev, 1)
        });
    });
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_dev_yog_NativeBridge_nativeOnUseItem<'l>(
    mut env: JNIEnv<'l>,
    _class: JClass<'l>,
    player: JString<'l>,
    item: JString<'l>,
    sneaking: jni::sys::jboolean,
    dimension: JString<'l>,
) {
    let (p, i, d) = (jstr!(env, player), jstr!(env, item), jstr!(env, dimension));
    let ev = yog_abi::YogUseItemEvent {
        player: YogStr::from_str(&p),
        item: YogStr::from_str(&i),
        sneaking: sneaking != 0,
        dimension: YogStr::from_str(&d),
    };
    let srv = srv_ptr();
    guard("on_use_item", || {
        each(&handlers().use_item, |ud, f| unsafe { f(ud, srv, &ev, 1) });
    });
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_dev_yog_NativeBridge_nativeOnUseItemPre<'l>(
    mut env: JNIEnv<'l>,
    _class: JClass<'l>,
    player: JString<'l>,
    item: JString<'l>,
    sneaking: jni::sys::jboolean,
    dimension: JString<'l>,
) -> jni::sys::jboolean {
    let h = handlers();
    if h.use_item.is_empty() {
        return 1;
    }
    let p = jstr!(env, player, 1);
    let i = jstr!(env, item, 1);
    let d = jstr!(env, dimension, 1);
    let ev = yog_abi::YogUseItemEvent {
        player: YogStr::from_str(&p),
        item: YogStr::from_str(&i),
        sneaking: sneaking != 0,
        dimension: YogStr::from_str(&d),
    };
    let srv = srv_ptr();
    allowed(&h.use_item, |ud, f| unsafe { f(ud, srv, &ev, 0) }) as jni::sys::jboolean
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_dev_yog_NativeBridge_nativeOnUseBlock<'l>(
    mut env: JNIEnv<'l>,
    _class: JClass<'l>,
    player: JString<'l>,
    block: JString<'l>,
    x: jint,
    y: jint,
    z: jint,
    dimension: JString<'l>,
    sneaking: jni::sys::jboolean,
) {
    let (p, b, d) = (jstr!(env, player), jstr!(env, block), jstr!(env, dimension));
    let ev = yog_abi::YogUseBlockEvent {
        player: YogStr::from_str(&p),
        block: YogStr::from_str(&b),
        pos: YogBlockPos { x, y, z },
        dimension: YogStr::from_str(&d),
        sneaking: sneaking != 0,
    };
    let srv = srv_ptr();
    guard("on_use_block", || {
        each(&handlers().use_block, |ud, f| unsafe { f(ud, srv, &ev, 1) });
    });
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_dev_yog_NativeBridge_nativeOnUseBlockPre<'l>(
    mut env: JNIEnv<'l>,
    _class: JClass<'l>,
    player: JString<'l>,
    block: JString<'l>,
    x: jint,
    y: jint,
    z: jint,
    dimension: JString<'l>,
    sneaking: jni::sys::jboolean,
) -> jni::sys::jboolean {
    let h = handlers();
    if h.use_block.is_empty() {
        return 1;
    }
    let p = jstr!(env, player, 1);
    let b = jstr!(env, block, 1);
    let d = jstr!(env, dimension, 1);
    let ev = yog_abi::YogUseBlockEvent {
        player: YogStr::from_str(&p),
        block: YogStr::from_str(&b),
        pos: YogBlockPos { x, y, z },
        dimension: YogStr::from_str(&d),
        sneaking: sneaking != 0,
    };
    let srv = srv_ptr();
    allowed(&h.use_block, |ud, f| unsafe { f(ud, srv, &ev, 0) }) as jni::sys::jboolean
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_dev_yog_NativeBridge_nativeOnAttackEntity<'l>(
    mut env: JNIEnv<'l>,
    _class: JClass<'l>,
    player: JString<'l>,
    target_type: JString<'l>,
    target_uuid: JString<'l>,
    dimension: JString<'l>,
) {
    let (p, tt, tu, d) = (
        jstr!(env, player),
        jstr!(env, target_type),
        jstr!(env, target_uuid),
        jstr!(env, dimension),
    );
    let ev = yog_abi::YogAttackEntityEvent {
        player: YogStr::from_str(&p),
        target_type: YogStr::from_str(&tt),
        target_uuid: YogStr::from_str(&tu),
        dimension: YogStr::from_str(&d),
    };
    let srv = srv_ptr();
    guard("on_attack_entity", || {
        each(&handlers().attack_entity, |ud, f| unsafe {
            f(ud, srv, &ev, 1)
        });
    });
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_dev_yog_NativeBridge_nativeOnEntityDamage<'l>(
    mut env: JNIEnv<'l>,
    _class: JClass<'l>,
    entity_type: JString<'l>,
    uuid: JString<'l>,
    amount: jfloat,
    source: JString<'l>,
    dimension: JString<'l>,
) {
    let (et, u, s, d) = (
        jstr!(env, entity_type),
        jstr!(env, uuid),
        jstr!(env, source),
        jstr!(env, dimension),
    );
    let ev = yog_abi::YogEntityDamageEvent {
        entity_type: YogStr::from_str(&et),
        uuid: YogStr::from_str(&u),
        amount,
        source: YogStr::from_str(&s),
        dimension: YogStr::from_str(&d),
    };
    let srv = srv_ptr();
    guard("on_entity_damage", || {
        each(&handlers().entity_damage, |ud, f| unsafe {
            f(ud, srv, &ev, 1)
        });
    });
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_dev_yog_NativeBridge_nativeOnEntityDeath<'l>(
    mut env: JNIEnv<'l>,
    _class: JClass<'l>,
    entity_type: JString<'l>,
    uuid: JString<'l>,
    source: JString<'l>,
    dimension: JString<'l>,
) {
    let (et, u, s, d) = (
        jstr!(env, entity_type),
        jstr!(env, uuid),
        jstr!(env, source),
        jstr!(env, dimension),
    );
    let ev = yog_abi::YogEntityDeathEvent {
        entity_type: YogStr::from_str(&et),
        uuid: YogStr::from_str(&u),
        source: YogStr::from_str(&s),
        dimension: YogStr::from_str(&d),
    };
    let srv = srv_ptr();
    guard("on_entity_death", || {
        each(&handlers().entity_death, |ud, f| unsafe {
            f(ud, srv, &ev, 1)
        });
    });
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_dev_yog_NativeBridge_nativeOnTick<'l>(
    _env: JNIEnv<'l>,
    _class: JClass<'l>,
) {
    let h = handlers();
    let srv = srv_ptr();
    guard("on_tick", || {
        each(&h.server_tick, |ud, f| unsafe { f(ud, srv) });
    });

    guard("startup_grants", process_pending_grants);

    // Entity data changed this tick, to who sees it — after the mods' ticks.
    guard("entity data", entity_data::flush);

    // Scheduler: what's due this tick, run outside the lock (a task may schedule more).
    let due = h.scheduler.lock().expect("scheduler poisoned").due();
    for (module, ud, f) in due {
        if module.is_retiring() {
            continue;
        }
        let Some(_call) = module.enter() else {
            continue;
        };
        guard("scheduled task", || unsafe { f(ud, srv) });
    }
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_dev_yog_NativeBridge_nativeOnServerStarted<'l>(
    _env: JNIEnv<'l>,
    _class: JClass<'l>,
    _world_dir: JString<'l>,
) {
    // Grants pending for a previous world's players are void.
    handlers().pending_grants.lock().expect("pending_grants poisoned").clear();
    let srv = srv_ptr();
    guard("on_server_started", || {
        each(&handlers().server_started, |ud, f| unsafe { f(ud, srv) });
    });
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_dev_yog_NativeBridge_nativeOnServerStopping<'l>(
    _env: JNIEnv<'l>,
    _class: JClass<'l>,
) {
    let srv = srv_ptr();
    guard("on_server_stopping", || {
        each(&handlers().server_stopping, |ud, f| unsafe { f(ud, srv) });
    });
    entity_data::server_stopped();
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_dev_yog_NativeBridge_nativeCommandNames<'l>(
    env: JNIEnv<'l>,
    _class: JClass<'l>,
) -> jstring {
    let names = handlers()
        .commands
        .keys()
        .cloned()
        .collect::<Vec<_>>()
        .join("\n");
    env.new_string(names)
        .map(jni::objects::JString::into_raw)
        .unwrap_or(std::ptr::null_mut())
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_dev_yog_NativeBridge_nativeOnBlockBreakPre<'l>(
    mut env: JNIEnv<'l>,
    _class: JClass<'l>,
    player: JString<'l>,
    block: JString<'l>,
    x: jint,
    y: jint,
    z: jint,
    dimension: JString<'l>,
) -> jni::sys::jboolean {
    let h = handlers();
    if h.block_break.is_empty() {
        return 1;
    }
    let p = jstr!(env, player, 1);
    let b = jstr!(env, block, 1);
    let d = jstr!(env, dimension, 1);
    let ev = yog_abi::YogBlockBreakEvent {
        player: YogStr::from_str(&p),
        block: YogStr::from_str(&b),
        pos: YogBlockPos { x, y, z },
        dimension: YogStr::from_str(&d),
    };
    let srv = srv_ptr();
    allowed(&h.block_break, |ud, f| unsafe { f(ud, srv, &ev, 0) }) as jni::sys::jboolean
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_dev_yog_NativeBridge_nativeOnChatPre<'l>(
    mut env: JNIEnv<'l>,
    _class: JClass<'l>,
    player: JString<'l>,
    message: JString<'l>,
    dimension: JString<'l>,
) -> jni::sys::jboolean {
    let h = handlers();
    if h.chat.is_empty() {
        return 1;
    }
    let p = jstr!(env, player, 1);
    let m = jstr!(env, message, 1);
    let d = jstr!(env, dimension, 1);
    let ev = yog_abi::YogChatEvent {
        player: YogStr::from_str(&p),
        message: YogStr::from_str(&m),
        dimension: YogStr::from_str(&d),
    };
    let srv = srv_ptr();
    allowed(&h.chat, |ud, f| unsafe { f(ud, srv, &ev, 0) }) as jni::sys::jboolean
}

/// Called by a registered chunk-generator closure (via `ChunkWriter::set_block`)
/// to place one block in the chunk currently being generated. Calls back
/// into `YogCallbackChunkGenerator.setBlockInGeneratingChunk` on the Java
/// side — same thread, since the whole round trip is synchronous.
unsafe extern "C" fn chunk_writer_set_block(
    _ctx: *mut c_void,
    x: i32,
    y: i32,
    z: i32,
    block_id: YogStr,
) -> bool {
    unsafe {
        host!(
            "dev/yog/YogCallbackChunkGenerator",
            "setBlockInGeneratingChunk",
            [I(x), I(y), I(z), S(block_id)]
        )
        .unwrap_or(false)
    }
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_dev_yog_NativeBridge_nativeGenerateChunk<'l>(
    mut env: JNIEnv<'l>,
    _class: JClass<'l>,
    generator_type_id: JString<'l>,
    chunk_x: jint,
    chunk_z: jint,
    min_y: jint,
    height: jint,
) {
    let id: String = match env.get_string(&generator_type_id) {
        Ok(s) => String::from(s),
        Err(_) => return,
    };
    let terrain = handlers().terrains.get(&id).cloned();
    if let Some(g) = terrain {
        guard("chunk_generator", || {
            g.generate(chunk_x, chunk_z, min_y, height, |x, y, z, block| unsafe {
                chunk_writer_set_block(std::ptr::null_mut(), x, y, z, YogStr::from_str(block));
            });
        });
        return;
    }
    let Some((module, ud, cb)) = handlers().chunk_generators.get(&id).cloned() else {
        return;
    };
    if module.is_retiring() {
        return;
    }
    let Some(_call) = module.enter() else { return };
    let writer = yog_abi::YogChunkWriterApi {
        chunk_x,
        chunk_z,
        min_y,
        height,
        ctx: std::ptr::null_mut(),
        set_block: chunk_writer_set_block,
    };
    guard("chunk_generator", || unsafe {
        cb(ud, &writer as *const _);
    });
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_dev_yog_NativeBridge_nativeTypedCommandSchemas<'l>(
    env: JNIEnv<'l>,
    _class: JClass<'l>,
) -> jstring {
    // One line per DISTINCT (name, schema) pair — a name can have several
    // typed registrations (different arg shapes); Brigadier merges repeated
    // `dispatcher.register()` calls for the same literal name into one tree,
    // so the Java side just needs every distinct schema once each.
    let mut lines = Vec::new();
    for (name, regs) in handlers().commands.iter() {
        let mut seen = std::collections::HashSet::new();
        for (_module, schema, _, _) in regs {
            if let Some(schema) = schema
                && seen.insert(schema.as_str())
            {
                lines.push(format!("{name}\t{schema}"));
            }
        }
    }
    env.new_string(lines.join("\n"))
        .map(jni::objects::JString::into_raw)
        .unwrap_or(std::ptr::null_mut())
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_dev_yog_NativeBridge_nativeOnCommand<'l>(
    mut env: JNIEnv<'l>,
    _class: JClass<'l>,
    name: JString<'l>,
    args: JString<'l>,
    source: JString<'l>,
    uuid: JString<'l>,
    dimension: JString<'l>,
) -> jstring {
    let (n, a, s, u, d) = (
        env.get_string(&name).map(String::from).unwrap_or_default(),
        env.get_string(&args).map(String::from).unwrap_or_default(),
        env.get_string(&source)
            .map(String::from)
            .unwrap_or_default(),
        env.get_string(&uuid).map(String::from).unwrap_or_default(),
        env.get_string(&dimension)
            .map(String::from)
            .unwrap_or_default(),
    );
    let h = handlers();
    let srv = srv_ptr();
    let args: Vec<&str> = if a.is_empty() { Vec::new() } else { a.split('\t').collect() };
    // One handler with `args`; its reply, or `None` when it passed.
    let call = |ud: &*mut c_void, f: &YogCommandFn, args: &str| -> Option<String> {
        let ev = yog_abi::YogCommandEvent {
            name: YogStr::from_str(&n),
            args: YogStr::from_str(args),
            source: YogStr::from_str(&s),
            uuid: YogStr::from_str(&u),
            dimension: YogStr::from_str(&d),
        };
        let mut buf = [0u8; 4096];
        let mut reply_len: u32 = 0;
        unsafe { f(*ud, srv, &ev, buf.as_mut_ptr(), buf.len() as u32, &mut reply_len) };
        (reply_len > 0).then(|| String::from_utf8_lossy(&buf[..reply_len as usize]).into_owned())
    };
    let reply = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let Some(regs) = h.commands.get(&n) else {
            return String::new();
        };
        let live = regs.iter().filter(|(module, ..)| !module.is_retiring());
        // 1. Typed handlers whose schema has exactly these many args.
        // 2. Ones ending in a `string`, with the extra words joined into it
        //    (the game may have parsed them as separate words).
        // 3. Plain (schema-less) handlers, with whatever was typed.
        for pass in 0..3 {
            for (module, schema, ud, f) in live.clone() {
                let joined;
                let these = match (pass, schema) {
                    (0, Some(schema)) if schema.split_whitespace().count() == args.len() => a.as_str(),
                    (1, Some(schema)) => {
                        let types: Vec<&str> = schema.split_whitespace().collect();
                        let k = types.len();
                        if types.last() != Some(&"string") || k == 0 || args.len() <= k {
                            continue;
                        }
                        let mut parts: Vec<String> = args[..k - 1].iter().map(|x| x.to_string()).collect();
                        parts.push(args[k - 1..].join(" "));
                        joined = parts.join("\t");
                        joined.as_str()
                    }
                    (2, None) => a.as_str(),
                    _ => continue,
                };
                let Some(_call) = module.enter() else {
                    continue;
                };
                if let Some(r) = call(ud, f, these) {
                    return r;
                }
            }
        }
        String::new()
    }))
    .unwrap_or_else(|_| {
        yog_logging::error!("a mod panicked handling command `{}`", n);
        String::new()
    });

    env.new_string(reply)
        .map(jni::objects::JString::into_raw)
        .unwrap_or(std::ptr::null_mut())
}

/// Everything the host registers as one Yog Data blob (see `content_blob`),
/// built on the first call and kept.
#[unsafe(no_mangle)]
pub extern "system" fn Java_dev_yog_NativeBridge_nativeContent<'l>(
    env: JNIEnv<'l>,
    _class: JClass<'l>,
) -> jni::sys::jbyteArray {
    let h = handlers();
    let bytes = {
        let mut cached = h.content_blob.lock().unwrap_or_else(|e| e.into_inner());
        cached.get_or_insert_with(|| std::sync::Arc::new(content_blob::blob(&h))).clone()
    };
    env.byte_array_from_slice(&bytes)
        .map(|a| a.into_raw())
        .unwrap_or(std::ptr::null_mut())
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_dev_yog_NativeBridge_nativeOnPacket<'l>(
    mut env: JNIEnv<'l>,
    _class: JClass<'l>,
    channel: JString<'l>,
    player: JString<'l>,
    payload: JByteArray<'l>,
) {
    let ch = env
        .get_string(&channel)
        .map(String::from)
        .unwrap_or_default();
    let pl = env
        .get_string(&player)
        .map(String::from)
        .unwrap_or_default();
    let data = env.convert_byte_array(&payload).unwrap_or_default();
    let ev = yog_abi::YogPacketEvent {
        channel: YogStr::from_str(&ch),
        player: YogStr::from_str(&pl),
        payload: data.as_ptr(),
        payload_len: data.len() as u32,
    };
    let h = handlers();
    let srv = srv_ptr();
    guard("on_packet", || {
        if let Some((module, ud, f)) = h.packets.get(&ch) {
            if module.is_retiring() {
                return;
            }
            let Some(_call) = module.enter() else { return };
            unsafe { f(*ud, srv, &ev) };
        }
    });
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_dev_yog_NativeBridge_nativeOnClientPacket<'l>(
    mut env: JNIEnv<'l>,
    _class: JClass<'l>,
    channel: JString<'l>,
    payload: JByteArray<'l>,
) {
    let ch = env
        .get_string(&channel)
        .map(String::from)
        .unwrap_or_default();
    let data = env.convert_byte_array(&payload).unwrap_or_default();
    if ch == entity_data::CHANNEL {
        guard("entity data", || entity_data::receive(&data));
        return;
    }
    let ev = yog_abi::YogPacketEvent {
        channel: YogStr::from_str(&ch),
        player: YogStr::EMPTY,
        payload: data.as_ptr(),
        payload_len: data.len() as u32,
    };
    let h = handlers();
    let srv = srv_ptr();
    guard("on_client_packet", || {
        if let Some((module, ud, f)) = h.client_packets.get(&ch) {
            if module.is_retiring() {
                return;
            }
            let Some(_call) = module.enter() else { return };
            unsafe { f(*ud, srv, &ev) };
        }
    });
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_dev_yog_NativeBridge_nativePacketChannels<'l>(
    env: JNIEnv<'l>,
    _class: JClass<'l>,
) -> jstring {
    let s = handlers()
        .packets
        .keys()
        .cloned()
        .collect::<Vec<_>>()
        .join("\n");
    env.new_string(s)
        .map(jni::objects::JString::into_raw)
        .unwrap_or(std::ptr::null_mut())
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_dev_yog_NativeBridge_nativeClientPacketChannels<'l>(
    env: JNIEnv<'l>,
    _class: JClass<'l>,
) -> jstring {
    let s = handlers()
        .client_packets
        .keys()
        .cloned()
        .chain([entity_data::CHANNEL.to_owned()])
        .collect::<Vec<_>>()
        .join("\n");
    env.new_string(s)
        .map(jni::objects::JString::into_raw)
        .unwrap_or(std::ptr::null_mut())
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_dev_yog_NativeBridge_nativeOnEntitySpawn<'l>(
    mut env: JNIEnv<'l>,
    _class: JClass<'l>,
    entity_type: JString<'l>,
    uuid: JString<'l>,
    dimension: JString<'l>,
) {
    let h = handlers();
    if h.entity_spawn.is_empty() {
        return;
    }
    let (et, u, d) = (
        jstr!(env, entity_type),
        jstr!(env, uuid),
        jstr!(env, dimension),
    );
    let ev = yog_abi::YogEntitySpawnEvent {
        entity_type: YogStr::from_str(&et),
        uuid: YogStr::from_str(&u),
        dimension: YogStr::from_str(&d),
    };
    let srv = srv_ptr();
    guard("on_entity_spawn", || {
        each(&h.entity_spawn, |ud, f| unsafe { f(ud, srv, &ev, 1) });
    });
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_dev_yog_NativeBridge_nativeOnEntitySpawnPre<'l>(
    mut env: JNIEnv<'l>,
    _class: JClass<'l>,
    entity_type: JString<'l>,
    uuid: JString<'l>,
    dimension: JString<'l>,
) -> jni::sys::jboolean {
    let h = handlers();
    if h.entity_spawn.is_empty() {
        return 1;
    }
    let et = jstr!(env, entity_type, 1);
    let u = jstr!(env, uuid, 1);
    let d = jstr!(env, dimension, 1);
    let ev = yog_abi::YogEntitySpawnEvent {
        entity_type: YogStr::from_str(&et),
        uuid: YogStr::from_str(&u),
        dimension: YogStr::from_str(&d),
    };
    let srv = srv_ptr();
    allowed(&h.entity_spawn, |ud, f| unsafe { f(ud, srv, &ev, 0) }) as jni::sys::jboolean
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_dev_yog_NativeBridge_nativeOnEntityDamagePre<'l>(
    mut env: JNIEnv<'l>,
    _class: JClass<'l>,
    entity_type: JString<'l>,
    uuid: JString<'l>,
    amount: jfloat,
    source: JString<'l>,
    dimension: JString<'l>,
) -> jni::sys::jboolean {
    let h = handlers();
    if h.entity_damage.is_empty() {
        return 1;
    }
    let et = jstr!(env, entity_type, 1);
    let u = jstr!(env, uuid, 1);
    let s = jstr!(env, source, 1);
    let d = jstr!(env, dimension, 1);
    let ev = yog_abi::YogEntityDamageEvent {
        entity_type: YogStr::from_str(&et),
        uuid: YogStr::from_str(&u),
        amount,
        source: YogStr::from_str(&s),
        dimension: YogStr::from_str(&d),
    };
    let srv = srv_ptr();
    allowed(&h.entity_damage, |ud, f| unsafe { f(ud, srv, &ev, 0) }) as jni::sys::jboolean
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_dev_yog_NativeBridge_nativeOnPlaceBlockPre<'l>(
    mut env: JNIEnv<'l>,
    _class: JClass<'l>,
    player: JString<'l>,
    block: JString<'l>,
    x: jint,
    y: jint,
    z: jint,
    dimension: JString<'l>,
    sneaking: jni::sys::jboolean,
) -> jni::sys::jboolean {
    let h = handlers();
    if h.player_place_block.is_empty() {
        return 1;
    }
    let p = jstr!(env, player, 1);
    let b = jstr!(env, block, 1);
    let d = jstr!(env, dimension, 1);
    let ev = YogPlaceBlockEvent {
        player: YogStr::from_str(&p),
        block: YogStr::from_str(&b),
        pos: YogBlockPos { x, y, z },
        dimension: YogStr::from_str(&d),
        sneaking: sneaking != 0,
    };
    let srv = srv_ptr();
    allowed(&h.player_place_block, |ud, f| unsafe { f(ud, srv, &ev, 0) }) as jni::sys::jboolean
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_dev_yog_NativeBridge_nativeOnPlaceBlock<'l>(
    mut env: JNIEnv<'l>,
    _class: JClass<'l>,
    player: JString<'l>,
    block: JString<'l>,
    x: jint,
    y: jint,
    z: jint,
    dimension: JString<'l>,
    sneaking: jni::sys::jboolean,
) {
    let h = handlers();
    if h.player_place_block.is_empty() {
        return;
    }
    let (p, b, d) = (jstr!(env, player), jstr!(env, block), jstr!(env, dimension));
    let ev = YogPlaceBlockEvent {
        player: YogStr::from_str(&p),
        block: YogStr::from_str(&b),
        pos: YogBlockPos { x, y, z },
        dimension: YogStr::from_str(&d),
        sneaking: sneaking != 0,
    };
    let srv = srv_ptr();
    guard("on_player_place_block", || {
        each(&h.player_place_block, |ud, f| unsafe { f(ud, srv, &ev, 1) });
    });
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_dev_yog_NativeBridge_nativeOnPlayerDeathPre<'l>(
    mut env: JNIEnv<'l>,
    _class: JClass<'l>,
    player: JString<'l>,
    uuid: JString<'l>,
    source: JString<'l>,
    dimension: JString<'l>,
) -> jni::sys::jboolean {
    let h = handlers();
    if h.player_death.is_empty() {
        return 1;
    }
    let p = jstr!(env, player, 1);
    let u = jstr!(env, uuid, 1);
    let s = jstr!(env, source, 1);
    let d = jstr!(env, dimension, 1);
    let ev = YogPlayerDeathEvent {
        player: YogStr::from_str(&p),
        uuid: YogStr::from_str(&u),
        source: YogStr::from_str(&s),
        dimension: YogStr::from_str(&d),
    };
    let srv = srv_ptr();
    allowed(&h.player_death, |ud, f| unsafe { f(ud, srv, &ev, 0) }) as jni::sys::jboolean
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_dev_yog_NativeBridge_nativeOnPlayerDeath<'l>(
    mut env: JNIEnv<'l>,
    _class: JClass<'l>,
    player: JString<'l>,
    uuid: JString<'l>,
    source: JString<'l>,
    dimension: JString<'l>,
) {
    let h = handlers();
    if h.player_death.is_empty() {
        return;
    }
    let (p, u, s, d) = (
        jstr!(env, player),
        jstr!(env, uuid),
        jstr!(env, source),
        jstr!(env, dimension),
    );
    let ev = YogPlayerDeathEvent {
        player: YogStr::from_str(&p),
        uuid: YogStr::from_str(&u),
        source: YogStr::from_str(&s),
        dimension: YogStr::from_str(&d),
    };
    let srv = srv_ptr();
    guard("on_player_death", || {
        each(&h.player_death, |ud, f| unsafe { f(ud, srv, &ev, 1) });
    });
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_dev_yog_NativeBridge_nativeOnPlayerRespawn<'l>(
    mut env: JNIEnv<'l>,
    _class: JClass<'l>,
    player: JString<'l>,
    uuid: JString<'l>,
    at_anchor: jni::sys::jboolean,
    dimension: JString<'l>,
) {
    let h = handlers();
    if h.player_respawn.is_empty() {
        return;
    }
    let (p, u, d) = (jstr!(env, player), jstr!(env, uuid), jstr!(env, dimension));
    let ev = YogPlayerRespawnEvent {
        player: YogStr::from_str(&p),
        uuid: YogStr::from_str(&u),
        at_anchor: at_anchor != 0,
        dimension: YogStr::from_str(&d),
    };
    let srv = srv_ptr();
    guard("on_player_respawn", || {
        each(&h.player_respawn, |ud, f| unsafe { f(ud, srv, &ev, 1) });
    });
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_dev_yog_NativeBridge_nativeOnAdvancement<'l>(
    mut env: JNIEnv<'l>,
    _class: JClass<'l>,
    player: JString<'l>,
    uuid: JString<'l>,
    advancement: JString<'l>,
    dimension: JString<'l>,
) {
    let h = handlers();
    if h.advancement.is_empty() {
        return;
    }
    let (p, u, a, d) = (
        jstr!(env, player),
        jstr!(env, uuid),
        jstr!(env, advancement),
        jstr!(env, dimension),
    );
    let ev = YogAdvancementEvent {
        player: YogStr::from_str(&p),
        uuid: YogStr::from_str(&u),
        advancement: YogStr::from_str(&a),
        dimension: YogStr::from_str(&d),
    };
    let srv = srv_ptr();
    guard("on_advancement", || {
        each(&h.advancement, |ud, f| unsafe { f(ud, srv, &ev, 1) });
    });
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_dev_yog_NativeBridge_nativeOnEntityInteractPre<'l>(
    mut env: JNIEnv<'l>,
    _class: JClass<'l>,
    player: JString<'l>,
    player_uuid: JString<'l>,
    entity_type: JString<'l>,
    entity_uuid: JString<'l>,
    hand: JString<'l>,
    dimension: JString<'l>,
) -> jni::sys::jboolean {
    let h = handlers();
    if h.entity_interact.is_empty() {
        return 1;
    }
    let p = jstr!(env, player, 1);
    let pu = jstr!(env, player_uuid, 1);
    let et = jstr!(env, entity_type, 1);
    let eu = jstr!(env, entity_uuid, 1);
    let ha = jstr!(env, hand, 1);
    let d = jstr!(env, dimension, 1);
    let ev = YogEntityInteractEvent {
        player: YogStr::from_str(&p),
        player_uuid: YogStr::from_str(&pu),
        entity_type: YogStr::from_str(&et),
        entity_uuid: YogStr::from_str(&eu),
        hand: YogStr::from_str(&ha),
        dimension: YogStr::from_str(&d),
    };
    let srv = srv_ptr();
    allowed(&h.entity_interact, |ud, f| unsafe { f(ud, srv, &ev, 0) }) as jni::sys::jboolean
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_dev_yog_NativeBridge_nativeOnEntityInteract<'l>(
    mut env: JNIEnv<'l>,
    _class: JClass<'l>,
    player: JString<'l>,
    player_uuid: JString<'l>,
    entity_type: JString<'l>,
    entity_uuid: JString<'l>,
    hand: JString<'l>,
    dimension: JString<'l>,
) {
    let h = handlers();
    if h.entity_interact.is_empty() {
        return;
    }
    let (p, pu) = (jstr!(env, player), jstr!(env, player_uuid));
    let (et, eu, ha, d) = (
        jstr!(env, entity_type),
        jstr!(env, entity_uuid),
        jstr!(env, hand),
        jstr!(env, dimension),
    );
    let ev = YogEntityInteractEvent {
        player: YogStr::from_str(&p),
        player_uuid: YogStr::from_str(&pu),
        entity_type: YogStr::from_str(&et),
        entity_uuid: YogStr::from_str(&eu),
        hand: YogStr::from_str(&ha),
        dimension: YogStr::from_str(&d),
    };
    let srv = srv_ptr();
    guard("on_entity_interact", || {
        each(&h.entity_interact, |ud, f| unsafe { f(ud, srv, &ev, 1) });
    });
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_dev_yog_NativeBridge_nativeOnItemCraft<'l>(
    mut env: JNIEnv<'l>,
    _class: JClass<'l>,
    player: JString<'l>,
    player_uuid: JString<'l>,
    result_item: JString<'l>,
    result_count: jint,
    dimension: JString<'l>,
) {
    let h = handlers();
    if h.item_craft.is_empty() {
        return;
    }
    let (p, pu, ri, d) = (
        jstr!(env, player),
        jstr!(env, player_uuid),
        jstr!(env, result_item),
        jstr!(env, dimension),
    );
    let ev = YogCraftEvent {
        player: YogStr::from_str(&p),
        player_uuid: YogStr::from_str(&pu),
        result_item: YogStr::from_str(&ri),
        result_count: result_count as u32,
        dimension: YogStr::from_str(&d),
    };
    let srv = srv_ptr();
    guard("on_item_craft", || {
        each(&h.item_craft, |ud, f| unsafe { f(ud, srv, &ev, 1) });
    });
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_dev_yog_NativeBridge_nativeOnExplosionPre<'l>(
    mut env: JNIEnv<'l>,
    _class: JClass<'l>,
    dimension: JString<'l>,
    x: jdouble,
    y: jdouble,
    z: jdouble,
    power: jfloat,
    cause_uuid: JString<'l>,
) -> jni::sys::jboolean {
    let h = handlers();
    if h.explosion.is_empty() {
        return 1;
    }
    let d = jstr!(env, dimension, 1);
    let cu = jstr!(env, cause_uuid, 1);
    let ev = YogExplosionEvent {
        dimension: YogStr::from_str(&d),
        x,
        y,
        z,
        power,
        cause_uuid: YogStr::from_str(&cu),
    };
    let srv = srv_ptr();
    allowed(&h.explosion, |ud, f| unsafe { f(ud, srv, &ev, 0) }) as jni::sys::jboolean
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_dev_yog_NativeBridge_nativeOnExplosion<'l>(
    mut env: JNIEnv<'l>,
    _class: JClass<'l>,
    dimension: JString<'l>,
    x: jdouble,
    y: jdouble,
    z: jdouble,
    power: jfloat,
    cause_uuid: JString<'l>,
) {
    let h = handlers();
    if h.explosion.is_empty() {
        return;
    }
    let (d, cu) = (jstr!(env, dimension), jstr!(env, cause_uuid));
    let ev = YogExplosionEvent {
        dimension: YogStr::from_str(&d),
        x,
        y,
        z,
        power,
        cause_uuid: YogStr::from_str(&cu),
    };
    let srv = srv_ptr();
    guard("on_explosion", || {
        each(&h.explosion, |ud, f| unsafe { f(ud, srv, &ev, 1) });
    });
}

// ── item insert JNI entry points ─────────────────────────────────────────────

/// A hopper or dropper offers an item to a block that accepts items:
/// `simulate` asks whether it would take it (Pre), else it takes it (Post).
#[unsafe(no_mangle)]
pub extern "system" fn Java_dev_yog_NativeBridge_nativeOnItemInsert<'l>(
    mut env: JNIEnv<'l>,
    _class: JClass<'l>,
    block: JString<'l>,
    x: jint,
    y: jint,
    z: jint,
    dimension: JString<'l>,
    item_id: JString<'l>,
    count: jint,
    nbt: JString<'l>,
    simulate: jni::sys::jboolean,
) -> jni::sys::jboolean {
    let h = handlers();
    if h.item_insert.is_empty() {
        return 0;
    }
    let (b, d, i, n) = (jstr!(env, block, 0), jstr!(env, dimension, 0), jstr!(env, item_id, 0), jstr!(env, nbt, 0));
    let ev = YogItemInsertEvent {
        block: YogStr::from_str(&b),
        pos: YogBlockPos { x, y, z },
        dimension: YogStr::from_str(&d),
        item_id: YogStr::from_str(&i),
        count: count.max(0) as u32,
        nbt: YogStr::from_str(&n),
    };
    let srv = srv_ptr();
    if simulate != 0 {
        // Some handler takes it.
        !allowed(&h.item_insert, |ud, f| unsafe { !f(ud, srv, &ev, 0) }) as jni::sys::jboolean
    } else {
        let mut taken = false;
        guard("on_item_insert", || {
            each(&h.item_insert, |ud, f| taken |= unsafe { f(ud, srv, &ev, 1) });
        });
        taken as jni::sys::jboolean
    }
}

// ── ABI minor 9 JNI entry points ─────────────────────────────────────────────

#[unsafe(no_mangle)]
pub extern "system" fn Java_dev_yog_NativeBridge_nativeOnItemPickupPre<'l>(
    mut env: JNIEnv<'l>,
    _class: JClass<'l>,
    player: JString<'l>,
    player_uuid: JString<'l>,
    item_id: JString<'l>,
    item_count: jint,
    entity_uuid: JString<'l>,
    dimension: JString<'l>,
) -> jni::sys::jboolean {
    let h = handlers();
    if h.item_pickup.is_empty() {
        return 1;
    }
    let p = jstr!(env, player, 1);
    let pu = jstr!(env, player_uuid, 1);
    let ii = jstr!(env, item_id, 1);
    let eu = jstr!(env, entity_uuid, 1);
    let d = jstr!(env, dimension, 1);
    let ev = YogItemPickupEvent {
        player: YogStr::from_str(&p),
        player_uuid: YogStr::from_str(&pu),
        item_id: YogStr::from_str(&ii),
        item_count: item_count as u32,
        entity_uuid: YogStr::from_str(&eu),
        dimension: YogStr::from_str(&d),
    };
    let srv = srv_ptr();
    allowed(&h.item_pickup, |ud, f| unsafe { f(ud, srv, &ev, 0) }) as jni::sys::jboolean
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_dev_yog_NativeBridge_nativeOnItemPickup<'l>(
    mut env: JNIEnv<'l>,
    _class: JClass<'l>,
    player: JString<'l>,
    player_uuid: JString<'l>,
    item_id: JString<'l>,
    item_count: jint,
    entity_uuid: JString<'l>,
    dimension: JString<'l>,
) {
    let h = handlers();
    if h.item_pickup.is_empty() {
        return;
    }
    let (p, pu) = (jstr!(env, player), jstr!(env, player_uuid));
    let (ii, eu, d) = (
        jstr!(env, item_id),
        jstr!(env, entity_uuid),
        jstr!(env, dimension),
    );
    let ev = YogItemPickupEvent {
        player: YogStr::from_str(&p),
        player_uuid: YogStr::from_str(&pu),
        item_id: YogStr::from_str(&ii),
        item_count: item_count as u32,
        entity_uuid: YogStr::from_str(&eu),
        dimension: YogStr::from_str(&d),
    };
    let srv = srv_ptr();
    guard("on_item_pickup", || {
        each(&h.item_pickup, |ud, f| unsafe { f(ud, srv, &ev, 1) });
    });
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_dev_yog_NativeBridge_nativeOnPlayerMove<'l>(
    mut env: JNIEnv<'l>,
    _class: JClass<'l>,
    player: JString<'l>,
    player_uuid: JString<'l>,
    x: jdouble,
    y: jdouble,
    z: jdouble,
    yaw: jfloat,
    pitch: jfloat,
    dimension: JString<'l>,
) {
    let h = handlers();
    if h.player_move.is_empty() {
        return;
    }
    let (p, pu, d) = (
        jstr!(env, player),
        jstr!(env, player_uuid),
        jstr!(env, dimension),
    );
    let ev = YogPlayerMoveEvent {
        player: YogStr::from_str(&p),
        player_uuid: YogStr::from_str(&pu),
        x,
        y,
        z,
        yaw,
        pitch,
        dimension: YogStr::from_str(&d),
    };
    let srv = srv_ptr();
    guard("on_player_move", || {
        each(&h.player_move, |ud, f| unsafe { f(ud, srv, &ev, 1) });
    });
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_dev_yog_NativeBridge_nativeOnContainerOpenPre<'l>(
    mut env: JNIEnv<'l>,
    _class: JClass<'l>,
    player: JString<'l>,
    player_uuid: JString<'l>,
    dimension: JString<'l>,
) -> jni::sys::jboolean {
    let h = handlers();
    if h.container_open.is_empty() {
        return 1;
    }
    let p = jstr!(env, player, 1);
    let pu = jstr!(env, player_uuid, 1);
    let d = jstr!(env, dimension, 1);
    let ev = YogContainerOpenEvent {
        player: YogStr::from_str(&p),
        player_uuid: YogStr::from_str(&pu),
        container_type: YogStr::EMPTY,
        dimension: YogStr::from_str(&d),
    };
    let srv = srv_ptr();
    allowed(&h.container_open, |ud, f| unsafe { f(ud, srv, &ev, 0) }) as jni::sys::jboolean
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_dev_yog_NativeBridge_nativeOnContainerOpen<'l>(
    mut env: JNIEnv<'l>,
    _class: JClass<'l>,
    player: JString<'l>,
    player_uuid: JString<'l>,
    container_type: JString<'l>,
    dimension: JString<'l>,
) {
    let h = handlers();
    if h.container_open.is_empty() {
        return;
    }
    let (p, pu, ct, d) = (
        jstr!(env, player),
        jstr!(env, player_uuid),
        jstr!(env, container_type),
        jstr!(env, dimension),
    );
    let ev = YogContainerOpenEvent {
        player: YogStr::from_str(&p),
        player_uuid: YogStr::from_str(&pu),
        container_type: YogStr::from_str(&ct),
        dimension: YogStr::from_str(&d),
    };
    let srv = srv_ptr();
    guard("on_container_open", || {
        each(&h.container_open, |ud, f| unsafe { f(ud, srv, &ev, 1) });
    });
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_dev_yog_NativeBridge_nativeOnContainerClose<'l>(
    mut env: JNIEnv<'l>,
    _class: JClass<'l>,
    player: JString<'l>,
    player_uuid: JString<'l>,
    dimension: JString<'l>,
) {
    let h = handlers();
    if h.container_close.is_empty() {
        return;
    }
    let (p, pu, d) = (
        jstr!(env, player),
        jstr!(env, player_uuid),
        jstr!(env, dimension),
    );
    let ev = YogContainerCloseEvent {
        player: YogStr::from_str(&p),
        player_uuid: YogStr::from_str(&pu),
        dimension: YogStr::from_str(&d),
    };
    let srv = srv_ptr();
    guard("on_container_close", || {
        each(&h.container_close, |ud, f| unsafe { f(ud, srv, &ev, 1) });
    });
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_dev_yog_NativeBridge_nativeOnProjectileHitPre<'l>(
    mut env: JNIEnv<'l>,
    _class: JClass<'l>,
    projectile_type: JString<'l>,
    projectile_uuid: JString<'l>,
    shooter_uuid: JString<'l>,
    hit_type: JString<'l>,
    hit_entity_uuid: JString<'l>,
    x: jdouble,
    y: jdouble,
    z: jdouble,
    dimension: JString<'l>,
) -> jni::sys::jboolean {
    let h = handlers();
    if h.projectile_hit.is_empty() {
        return 1;
    }
    let pt = jstr!(env, projectile_type, 1);
    let pu = jstr!(env, projectile_uuid, 1);
    let su = jstr!(env, shooter_uuid, 1);
    let ht = jstr!(env, hit_type, 1);
    let heu = jstr!(env, hit_entity_uuid, 1);
    let dim = jstr!(env, dimension, 1);
    let ev = YogProjectileHitEvent {
        projectile_type: YogStr::from_str(&pt),
        projectile_uuid: YogStr::from_str(&pu),
        shooter_uuid: YogStr::from_str(&su),
        hit_type: YogStr::from_str(&ht),
        hit_entity_uuid: YogStr::from_str(&heu),
        x,
        y,
        z,
        dimension: YogStr::from_str(&dim),
    };
    let srv = srv_ptr();
    allowed(&h.projectile_hit, |ud, f| unsafe { f(ud, srv, &ev, 0) }) as jni::sys::jboolean
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_dev_yog_NativeBridge_nativeOnProjectileHit<'l>(
    mut env: JNIEnv<'l>,
    _class: JClass<'l>,
    projectile_type: JString<'l>,
    projectile_uuid: JString<'l>,
    shooter_uuid: JString<'l>,
    hit_type: JString<'l>,
    hit_entity_uuid: JString<'l>,
    x: jdouble,
    y: jdouble,
    z: jdouble,
    dimension: JString<'l>,
) {
    let h = handlers();
    if h.projectile_hit.is_empty() {
        return;
    }
    let (pt, pu) = (jstr!(env, projectile_type), jstr!(env, projectile_uuid));
    let (su, ht) = (jstr!(env, shooter_uuid), jstr!(env, hit_type));
    let (heu, dim) = (jstr!(env, hit_entity_uuid), jstr!(env, dimension));
    let ev = YogProjectileHitEvent {
        projectile_type: YogStr::from_str(&pt),
        projectile_uuid: YogStr::from_str(&pu),
        shooter_uuid: YogStr::from_str(&su),
        hit_type: YogStr::from_str(&ht),
        hit_entity_uuid: YogStr::from_str(&heu),
        x,
        y,
        z,
        dimension: YogStr::from_str(&dim),
    };
    let srv = srv_ptr();
    guard("on_projectile_hit", || {
        each(&h.projectile_hit, |ud, f| unsafe { f(ud, srv, &ev, 1) });
    });
}

// ── ABI minor 10 — client-side JNI entry points ───────────────────────────────

#[unsafe(no_mangle)]
pub extern "system" fn Java_dev_yog_NativeBridge_nativeOnClientTick<'l>(
    _env: JNIEnv<'l>,
    _class: JClass<'l>,
) {
    let h = handlers();
    if h.client_tick.is_empty() {
        return;
    }
    guard("on_client_tick", || {
        each(&h.client_tick, |ud, f| unsafe { f(ud) });
    });
}

/// The host's GL function pointer for `sym` (0 if it has none).
fn gl_proc_address(sym: &str) -> usize {
    let ptr: Option<i64> = unsafe { bridge!("glProcAddress", [S(YogStr::from_str(sym))]) };
    ptr.unwrap_or(0) as usize
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_dev_yog_NativeBridge_nativeGlInit<'l>(
    _env: JNIEnv<'l>,
    _class: JClass<'l>,
) {
    if GL.get().is_some() {
        return;
    }
    let mut raw_get_binary: usize = 0;
    let mut raw_prog_binary: usize = 0;
    let gl = unsafe {
        glow::Context::from_loader_function(|sym| {
            let ptr = gl_proc_address(sym);
            // Capture extension pointers while the loader runs.
            match sym {
                "glGetProgramBinary" => raw_get_binary = ptr,
                "glProgramBinary" => raw_prog_binary = ptr,
                _ => {}
            }
            ptr as *const _
        })
    };
    let _ = GL.set(GlCtx(gl));
    let _ = GL_GET_PROGRAM_BINARY.set((raw_get_binary != 0).then_some(raw_get_binary));
    let _ = GL_PROGRAM_BINARY.set((raw_prog_binary != 0).then_some(raw_prog_binary));
    // `glGetProgramiv` is a core function; always available.  We look it up once here
    // to avoid depending on glow internals for the PROGRAM_BINARY_LENGTH query.
    let get_program_iv = gl_proc_address("glGetProgramiv");
    let _ = GL_GET_PROGRAM_IV.set((get_program_iv != 0).then_some(get_program_iv));
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_dev_yog_NativeBridge_nativeOnHudRender<'l>(
    _env: JNIEnv<'l>,
    _class: JClass<'l>,
    delta_tick: jfloat,
    screen_w: jint,
    screen_h: jint,
    scale_factor: jfloat,
    player_x: jfloat,
    player_y: jfloat,
    player_z: jfloat,
) {
    let h = handlers();
    let mut gfx = GFX_FN_TABLE;
    gfx.screen_w = screen_w;
    gfx.screen_h = screen_h;
    gfx.delta_tick = delta_tick;
    gfx.scale_factor = scale_factor;
    gfx.player_pos = [player_x, player_y, player_z];

    // Mod hud_render callbacks.
    if !h.hud_render.is_empty() {
        guard("on_hud_render", || {
            each(&h.hud_render, |ud, f| unsafe { f(ud, &gfx) });
        });
    }

    // Render active book UIs.
    let active_layers: Vec<UiLayer> = h.active_uis.lock().unwrap().clone();
    let sw = screen_w as f32;
    let sh = screen_h as f32;
    let ctx = unsafe { yog_gfx::GfxContext::from_raw(&gfx as *const _) };
    let mut renderers = h.book_renderers.lock().unwrap();
    for layer in &active_layers {
        if !layer.visible {
            continue;
        }
        if let Some(renderer) = renderers.get_mut(&layer.id) {
            renderer.render(&ctx, sw, sh);
        }
    }
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_dev_yog_NativeBridge_nativeOnWorldRender<'l>(
    env: JNIEnv<'l>,
    _class: JClass<'l>,
    delta_tick: jfloat,
    screen_w: jint,
    screen_h: jint,
    scale_factor: jfloat,
    view_proj_arr: JFloatArray<'l>,
    cam_x: jfloat,
    cam_y: jfloat,
    cam_z: jfloat,
    player_x: jfloat,
    player_y: jfloat,
    player_z: jfloat,
) {
    let h = handlers();
    if h.world_render.is_empty() {
        return;
    }
    let mut view_proj = [0f32; 16];
    if env
        .get_float_array_region(&view_proj_arr, 0, &mut view_proj)
        .is_err()
    {
        return;
    }
    let mut gfx = GFX_FN_TABLE;
    gfx.screen_w = screen_w;
    gfx.screen_h = screen_h;
    gfx.delta_tick = delta_tick;
    gfx.scale_factor = scale_factor;
    gfx.view_proj = view_proj;
    gfx.camera_pos = [cam_x, cam_y, cam_z];
    gfx.player_pos = [player_x, player_y, player_z];
    guard("on_world_render", || {
        each(&h.world_render, |ud, f| unsafe { f(ud, &gfx) });
    });
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_dev_yog_NativeBridge_nativeOnKeyPress<'l>(
    _env: JNIEnv<'l>,
    _class: JClass<'l>,
    key_code: jint,
    scan_code: jint,
    action: jint,
    modifiers: jint,
) -> jni::sys::jboolean {
    let h = handlers();
    if h.key_press.is_empty() {
        return 1;
    }
    let ev = YogKeyPressEvent {
        key_code,
        scan_code,
        action,
        modifiers,
    };
    allowed(&h.key_press, |ud, f| unsafe { f(ud, &ev) }) as jni::sys::jboolean
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_dev_yog_NativeBridge_nativeOnScreenOpen<'l>(
    mut env: JNIEnv<'l>,
    _class: JClass<'l>,
    screen_class: JString<'l>,
) {
    let h = handlers();
    if h.screen_open.is_empty() {
        return;
    }
    let sc = match env.get_string(&screen_class) {
        Ok(s) => String::from(s),
        Err(_) => return,
    };
    guard("on_screen_open", || {
        each(&h.screen_open, |ud, f| unsafe {
            f(ud, YogStr::from_str(&sc))
        });
    });
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_dev_yog_NativeBridge_nativeOnScreenClose<'l>(
    mut env: JNIEnv<'l>,
    _class: JClass<'l>,
    screen_class: JString<'l>,
) {
    let h = handlers();
    if h.screen_close.is_empty() {
        return;
    }
    let sc = match env.get_string(&screen_class) {
        Ok(s) => String::from(s),
        Err(_) => return,
    };
    guard("on_screen_close", || {
        each(&h.screen_close, |ud, f| unsafe {
            f(ud, YogStr::from_str(&sc))
        });
    });
}

mod body;
mod entity_data;
mod client_api;
mod content_blob;
mod machine;
mod control_socket;
mod images;
mod svg;
mod jcall;
mod ui_jni;
// Entry points for the benchmarks only: never in a build of the loader.
#[cfg(feature = "bench")]
#[doc(hidden)]
pub mod bench_support;
#[cfg(test)]
mod tests;
