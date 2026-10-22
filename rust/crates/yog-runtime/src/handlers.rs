//! The runtime's handler storage: what mods registered, read during dispatch.
use crate::globals::HANDLERS;
use crate::globals::HOT_RELOADER;
use std::collections::HashMap;
use yog_hot_reload::HotReloader;
use yog_hot_reload::ModuleGeneration;
use yog_hot_reload::ModuleRegistry;
use yog_registry::BlockDef;
use yog_registry::ItemDef;
use yog_sync::Arc;
use yog_sync::Mutex;

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
    /// (namespace, name, the recipe in Yog's own format).
    pub(super) recipes: Vec<(String, String, yog_data::Value)>,
    pub(super) items: Vec<ItemDef>,
    /// Mods' entity types, attributes, mob effects and potions: the hosts get them whole.
    pub(super) entities: Vec<yog_registry::EntityDef>,
    pub(super) attributes: Vec<yog_registry::AttributeDef>,
    pub(super) effects: Vec<yog_registry::EffectDef>,
    pub(super) potions: Vec<yog_registry::PotionDef>,
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
    /// Dimensions' `terrain`s (`.ydef`), generated here: by the dimension's id,
    /// which is its `generator_type`.
    pub(super) terrains: HashMap<String, Arc<yog_worldgen::Generator>>,
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

pub(super) fn handlers() -> yog_sync::RwLockReadGuard<'static, RuntimeHandlers> {
    HANDLERS.read()
}

/// The handlers, to change (content a mod registers).
pub(super) fn handlers_write() -> yog_sync::RwLockWriteGuard<'static, RuntimeHandlers> {
    HANDLERS.write()
}

pub(super) fn hot_reloader() -> &'static HotReloader<RuntimeModuleRegistry> {
    &HOT_RELOADER
}

/// The `yog-hot-reload` host contract: `retarget` needs no work here because
/// `LoadedModule::is_retiring()` already makes a superseded generation's
/// handlers inert (the router checks it) the instant `HotReloader::reload`
/// calls `retire` on the old module — well before this is even invoked.
/// `purge` is the actual safety-net sweep, dropping every bus handler of a
/// retired generation right before its library is hard-unloaded.
pub(super) struct RuntimeModuleRegistry;

impl ModuleRegistry for RuntimeModuleRegistry {
    fn retarget(&self, _mod_id: &str, _from: ModuleGeneration, _to: ModuleGeneration) {}

    fn purge(&self, generation: ModuleGeneration) {
        crate::bus::ROUTER.purge(generation);
        crate::bus::purge_descriptors(generation);
        crate::mirrors::purge(generation);
    }
}
