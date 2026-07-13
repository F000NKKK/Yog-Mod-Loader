//! Yog API — the single crate mod authors depend on.
//!
//! A facade that re-exports every Yog domain plus the central [`Registry`] hub.
//! Add a new domain crate, re-export it here, and mods pick it up via
//! `yog_api::*`. Items are available both flat (`yog_api::Registry`) and
//! namespaced by domain (`yog_api::world::World`).

mod interop;
mod registry;

pub use interop::Interop;
pub use registry::{installed_mods, open_ui, server, CServer, Mod, ModInfo, Registry};
pub use yog_gfx::{core as gfx_core, draw2d as gfx_draw2d, gl as gfx_gl, GfxContext};

/// Stable C ABI — re-exported so mods don't need a direct `yog-abi` dependency.
pub use yog_abi::{YogApi, ABI_VERSION};

/// Inter-mod communication proc-macros.
pub use yog_interop::{import, yog_export};

/// rkyv — zero-copy serialization for interop. Re-exported so generated
/// code and mods can use `rkyv::Archive`, `rkyv::Serialize`, `rkyv::Deserialize`
/// without a direct dependency.
pub use rkyv;

/// Re-exported so `#[yog_export]`-generated code can emit `#[::yog_api::ctor::ctor(unsafe)]`
/// without mods needing a direct `ctor` dependency. Constructors run when the
/// mod's `cdylib` is loaded — guaranteed to complete before the runtime calls
/// `yog_mod_register`, so the export registry is fully populated by then.
pub use ctor;

#[doc(hidden)]
pub use std::os::raw::c_void as __c_void;

/// Export a [`Mod`] as a dynamically loadable Yog mod.
///
/// Generates the two C-ABI entry points the runtime looks up:
/// - `yog_abi_version() -> u32`  — version check before loading
/// - `yog_mod_register(*const YogApi, *const c_char)` — registration entry point,
///   receives the mod's `id` from its manifest
///
/// Put this once at the crate root of a `cdylib` mod:
///
/// ```ignore
/// yog_api::export_mod!(MyMod);
/// ```
#[macro_export]
macro_rules! export_mod {
    ($mod_ty:ty) => {
        #[no_mangle]
        pub extern "C" fn yog_abi_version() -> u32 {
            $crate::ABI_VERSION
        }

        #[no_mangle]
        pub unsafe extern "C" fn yog_mod_register(
            api: *const $crate::YogApi,
            mod_id_ptr: *const ::std::os::raw::c_char,
        ) {
            // Parse mod_id from the C string passed by the runtime.
            let mod_id: &str = if mod_id_ptr.is_null() {
                "unknown"
            } else {
                match ::std::ffi::CStr::from_ptr(mod_id_ptr).to_str() {
                    Ok(s) => s,
                    Err(_) => "unknown",
                }
            };
            // Store for interop use (yog_api::interop::current_mod_id()).
            $crate::__set_current_mod_id(mod_id);
            // Every mod is handed the same global `YogApi` table, so any
            // mod's own pointer works for reconstructing a `Registry` inside
            // an exported function later — even when called from a thread
            // that never ran `yog_mod_register` itself.
            $crate::__set_current_api_ptr(api as usize);

            // Catch panics so they never unwind across this `extern "C"` boundary
            // back into the runtime (which would be undefined behaviour).
            let outcome = ::std::panic::catch_unwind(::std::panic::AssertUnwindSafe(|| {
                // SAFETY: the runtime passes a valid YogApi pointer, verified via
                // yog_abi_version() and abi_version/size checks before this call.
                let mut registry = unsafe { $crate::Registry::from_raw(api) };
                <$mod_ty as $crate::Mod>::register(&mut registry);

                // Auto-register all #[yog_export] functions (populated by
                // per-function `ctor` constructors at cdylib load time).
                for entry in $crate::__yog_export_registry().lock().unwrap().iter() {
                    registry
                        .interop()
                        .export(entry.name, entry.ptr as *const ::std::os::raw::c_void);
                }

                // Resolve pending imports (may partially succeed — runtime
                // does a final pass after all mods loaded).
                $crate::__yog_resolve_pending_imports(&registry);
            }));
            if outcome.is_err() {
                $crate::error!(
                    "mod {} panicked during register",
                    ::core::stringify!($mod_ty)
                );
            }
        }

        /// Called by the runtime after ALL mods are loaded — resolves any
        /// imports that weren't satisfied during initial registration.
        #[no_mangle]
        pub unsafe extern "C" fn __yog_resolve_imports_final(api: *const $crate::YogApi) {
            let registry = unsafe { $crate::Registry::from_raw(api) };
            $crate::__yog_resolve_pending_imports(&registry);
        }
    };
}

/// Internal: set by `export_mod!` before calling `Mod::register`.
/// Used by `yog_api::interop::current_mod_id()` so `Interop::export` knows
/// which mod is calling.
#[doc(hidden)]
pub fn __set_current_mod_id(id: &str) {
    CURRENT_MOD_ID.with(|cell| cell.replace(Some(id.to_string())));
}

/// Internal: the current mod's id, set during `yog_mod_register`.
#[doc(hidden)]
pub fn __current_mod_id() -> Option<String> {
    CURRENT_MOD_ID.with(|cell| cell.borrow().clone())
}

/// Internal: the global `YogApi` pointer, set by every mod's `yog_mod_register`
/// (it's the same singleton table for the whole process). Process-wide (not
/// thread-local) because `#[yog_export]`-generated wrappers read it whenever
/// an exported function is *called*, which may happen on any thread, long
/// after the loading thread's `yog_mod_register` returned.
#[doc(hidden)]
pub fn __set_current_api_ptr(ptr: usize) {
    API_PTR.store(ptr, std::sync::atomic::Ordering::Release);
}

#[doc(hidden)]
pub fn __current_api_ptr() -> usize {
    API_PTR.load(std::sync::atomic::Ordering::Acquire)
}

static API_PTR: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);

std::thread_local! {
    static CURRENT_MOD_ID: std::cell::RefCell<Option<String>> = std::cell::RefCell::new(None);
}

// ── Auto-export registry ─────────────────────────────────────────────────

#[doc(hidden)]
pub struct YogExportEntry {
    pub name: &'static str,
    pub ptr: usize,
}

#[doc(hidden)]
pub struct YogImportEntry {
    pub mod_id: &'static str,
    pub symbol: &'static str,
    pub bind_fn: usize,
}

#[doc(hidden)]
pub fn __yog_export_registry() -> &'static std::sync::Mutex<Vec<YogExportEntry>> {
    static REG: std::sync::LazyLock<std::sync::Mutex<Vec<YogExportEntry>>> =
        std::sync::LazyLock::new(|| std::sync::Mutex::new(Vec::new()));
    &REG
}

#[doc(hidden)]
pub fn __yog_import_registry() -> &'static std::sync::Mutex<Vec<YogImportEntry>> {
    static REG: std::sync::LazyLock<std::sync::Mutex<Vec<YogImportEntry>>> =
        std::sync::LazyLock::new(|| std::sync::Mutex::new(Vec::new()));
    &REG
}

/// Called by `export_mod!` after registration. Iterates pending imports and
/// resolves them via the interop table. Also exported as `#[no_mangle]` so the
/// runtime can call it after all mods are loaded (final resolution pass).
#[doc(hidden)]
pub unsafe fn __yog_resolve_pending_imports(registry: &crate::Registry) {
    let interop = registry.interop();
    for entry in __yog_import_registry().lock().unwrap().iter() {
        if let Some(ptr) = interop.import_raw(entry.mod_id, entry.symbol) {
            let bind: unsafe extern "C" fn(*const std::ffi::c_void) =
                std::mem::transmute(entry.bind_fn);
            bind(ptr);
        }
    }
}

pub use yog_book::{
    crafting_page, entity_page, image_page, pattern_page, relations_page, smelting_page,
    spotlight_page, text_page, text_page_titled,
};
pub use yog_book::{Book, BookCategory, BookEntry, BookMacro, BookPage, BookRegistry};
pub use yog_book::{BookFontRegistry, BookRenderer};
pub use yog_command::CommandContext;
pub use yog_config::Config;
pub use yog_core::{BlockPos, Server};
pub use yog_entity::Entity;
pub use yog_event::{
    AdvancementEvent, AttackEntityEvent, BlockBreakEvent, ChatEvent, ClientTickEvent,
    ContainerCloseEvent, ContainerOpenEvent, CraftEvent, EntityDamageEvent, EntityDeathEvent,
    EntityInteractEvent, EntitySpawnEvent, EventPhase, ExplosionEvent, ItemPickupEvent,
    KeyPressEvent, PlaceBlockEvent, PlayerDeathEvent, PlayerJoinEvent, PlayerLeaveEvent,
    PlayerMoveEvent, PlayerRespawnEvent, ProjectileHitEvent, ScreenEvent, UseBlockEvent,
    UseItemEvent,
};
pub use yog_inventory::{InventoryDef, SlotLayout};
#[doc(inline)]
pub use yog_network::packet;
pub use yog_network::{Packet, PacketEvent, PacketField};
pub use yog_player::Player;
pub use yog_registry::{
    AdvancementReward, BlockDef, BookRecipe, FoodDef, FurnaceRecipe, ItemDef, ItemModifier,
    ShapedRecipe, ShapelessRecipe, StartupGrant,
};
pub use yog_storage::{Storage, StorageScope, Value};
pub use yog_ui::{widget, Align, Dock, FlexDir, FocusStyle, LayoutNode, Rect, UiRoot};
pub use yog_world::World;

/// Logging macros (`yog_api::info!`, `warn!`, `error!`).
pub use yog_logging::{error, info, warn};

/// Core types and handles.
pub mod core {
    pub use yog_core::*;
}

/// Events and the subscription registry.
pub mod event {
    pub use yog_event::*;
}

/// World access (block get/set, dimensions).
pub mod world {
    pub use yog_world::*;
}

/// Entity access (teleport, position, health, ... by UUID).
pub mod entity {
    pub use yog_entity::*;
}

/// Player access (give item, teleport).
pub mod player {
    pub use yog_player::*;
}

/// Content registration (custom items / blocks / food).
pub mod content {
    pub use yog_registry::*;
}

/// Networking (raw-byte packets over channels).
pub mod network {
    pub use yog_network::*;
}

/// Commands.
pub mod command {
    pub use yog_command::*;
}

/// Persistent key-value storage for mod data.
pub mod storage {
    pub use yog_storage::*;
}

/// Mod configuration (typed key/value files).
pub mod config {
    pub use yog_config::*;
}

/// In-game book/documentation system (Patchouli-like).
pub mod book {
    pub use yog_book::*;
}

/// UI framework — flexbox layout + widgets on top of yog-gfx.
pub mod ui {
    pub use yog_ui::*;
}

/// Inventory framework — real Container/Menu screens (BlockEntity-backed),
/// as opposed to `ui`'s HUD-drawn overlays. See `yog-inventory`'s DESIGN.md.
pub mod inventory {
    pub use yog_inventory::*;
}
