//! The runtime's handler storage: what mods registered, read during dispatch.

use super::*;

// ── Handler storage ───────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct UiLayer {
    pub id: String,
    pub parent: Option<String>,
    pub modal: bool,
    pub visible: bool,
    pub z_index: i32,
}

/// One command registration: owning module, typed-args schema (if any),
/// user data and callback.
/// A tooltip picture's mod, user data, size and draw callbacks.
pub(super) type TooltipEntry = (LoadedModule, *mut c_void, yog_abi::YogTooltipSizeFn, yog_abi::YogTooltipDrawFn);

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
pub(super) struct RuntimeHandlers {
    pub(super) hud_render: Vec<(LoadedModule, *mut c_void, YogHudRenderFn)>,
    pub(super) world_render: Vec<(LoadedModule, *mut c_void, YogWorldRenderFn)>,
    /// (namespace, name, the recipe in Yog's own format).
    pub(super) recipes: Vec<(String, String, yog_data::Value)>,
    pub(super) items: Vec<ItemDef>,
    /// Mods' entity types, as Yog Data (the host reads them whole).
    pub(super) entities: Vec<yog_data::Value>,
    /// Mods' attributes, mob effects and potions, as Yog Data (read whole too).
    pub(super) attributes: Vec<yog_data::Value>,
    pub(super) effects: Vec<yog_data::Value>,
    pub(super) potions: Vec<yog_data::Value>,
    pub(super) blocks: Vec<BlockDef>,
    /// The content blob the hosts read (`nativeContent`), built once —
    /// dropped when more content registers.
    pub(super) content_blob: Mutex<Option<Arc<Vec<u8>>>>,
    /// Mods' sound groups by id; a block's `sound` is one of these or built in.
    pub(super) sound_groups: HashMap<String, yog_registry::SoundGroup>,
    /// Mods' tool and armor materials by id (the game's are built in).
    pub(super) tool_materials: HashMap<String, yog_registry::ToolMaterial>,
    /// Placed features added to biomes (an ore's): for the hosts that add them in code.
    pub(super) biome_features: Vec<yog_registry::BiomeFeature>,
    pub(super) armor_materials: HashMap<String, yog_registry::ArmorMaterial>,
    pub(super) inventories: Vec<yog_inventory::InventoryDef>,
    /// The ids of the registered books (their renderers are `book_renderers`).
    pub(super) books: std::collections::HashSet<String>,
    pub(super) book_renderers: Mutex<HashMap<String, yog_book::BookRenderer>>,
    /// Declared dimension-type defs: id -> `YogDimensionDef` JSON. Actual
    /// `LevelStem`/`ServerLevel` creation is a host (Java) responsibility —
    /// see `yog-dimensions` crate docs.
    pub(super) dimensions: HashMap<String, yog_data::Value>,
    pub(super) chunk_generators: HashMap<String, (LoadedModule, *mut c_void, yog_abi::YogChunkGeneratorFn)>,
    /// Dimensions' `terrain`s (`.ydef`), generated here: by the dimension's id,
    /// which is its `generator_type`.
    pub(super) terrains: HashMap<String, Arc<yog_worldgen::Generator>>,
    pub(super) ui_handlers: HashMap<String, (LoadedModule, *mut c_void, yog_abi::YogUIEventFn)>,
    pub(super) ui_render_handlers: HashMap<String, Vec<(LoadedModule, *mut c_void, YogHudRenderFn)>>,
    /// Book pages' pictures (`BookPage::Drawn`), by drawer id.
    pub(super) page_drawers: HashMap<String, (LoadedModule, *mut c_void, yog_abi::YogPageDrawFn)>,
    /// Pictures in item tooltips, by item id: the first that answers draws.
    pub(super) item_tooltips: HashMap<String, Vec<TooltipEntry>>,
    pub(super) menu_entries: Vec<(String, String)>, // (label, ui_id) for vanilla screen buttons
    /// The mods' key bindings (ABI 3.10), once each by id: the game's controls show them.
    pub(super) key_bindings: Vec<KeyBinding>,
    pub active_uis: Mutex<Vec<UiLayer>>,
    pub(super) startup_grants: Vec<yog_registry::StartupGrant>,
    /// Players whose startup grants are pending: (name, uuid, ticks_waited).
    /// Granting happens on a server tick after join — giving items directly in
    /// the join callback fails for the host of a freshly created world.
    pub(super) pending_grants: Mutex<Vec<(String, String, u32)>>,
}

// All fn ptrs are C-ABI; ud pointers are from Box::into_raw of Send+Sync closures.
unsafe impl Send for RuntimeHandlers {}
unsafe impl Sync for RuntimeHandlers {}

/// A mod callback, tagged with the module that registered it.
pub(super) type Entry<F> = (LoadedModule, *mut c_void, F);

/// Calls `call(ud, f)` for every entry whose module is still live, until
/// one returns `false`; whether none did.
pub(super) fn each_while<F: Copy>(entries: &[Entry<F>], mut call: impl FnMut(*mut c_void, F) -> bool) -> bool {
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
pub(super) fn each<F: Copy, R>(entries: &[Entry<F>], mut call: impl FnMut(*mut c_void, F) -> R) {
    each_while(entries, |ud, f| {
        call(ud, f);
        true
    });
}

pub(super) fn handlers() -> std::sync::RwLockReadGuard<'static, RuntimeHandlers> {
    HANDLERS
        .get()
        .expect("yog: nativeInit not called yet")
        .read_locked()
}

pub(super) fn current_registration() -> LoadedModule {
    CURRENT_REGISTRATION
        .locked()
        .clone()
        .expect("a yog-runtime api_* function was called outside of yog_mod_register")
}

pub(super) fn hot_reloader() -> &'static HotReloader<RuntimeModuleRegistry> {
    HOT_RELOADER.get().expect("yog: nativeInit not called yet")
}


pub(super) struct RuntimeModuleRegistry;

impl ModuleRegistry for RuntimeModuleRegistry {
    fn retarget(&self, _mod_id: &str, _from: ModuleGeneration, _to: ModuleGeneration) {}

    fn purge(&self, generation: ModuleGeneration) {
        crate::bus::ROUTER.purge(generation);
        let Some(lock) = HANDLERS.get() else { return };
        let mut h = lock.write_locked();
        let h = &mut *h;
        for_each_handler_list!(h, |list| list
            .retain(|(m, ..)| m.generation() != generation));
        h.chunk_generators
            .retain(|_, (m, ..)| m.generation() != generation);
        h.ui_handlers
            .retain(|_, (m, ..)| m.generation() != generation);
        for entries in h.ui_render_handlers.values_mut() {
            entries.retain(|(m, ..)| m.generation() != generation);
        }
        h.page_drawers
            .retain(|_, (m, ..)| m.generation() != generation);
        for entries in h.item_tooltips.values_mut() {
            entries.retain(|(m, ..)| m.generation() != generation);
        }
    }
}
