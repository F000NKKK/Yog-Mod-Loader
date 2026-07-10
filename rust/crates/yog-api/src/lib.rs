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
pub use yog_gfx::{GfxContext, core as gfx_core, gl as gfx_gl, draw2d as gfx_draw2d};

/// Stable C ABI — re-exported so mods don't need a direct `yog-abi` dependency.
pub use yog_abi::{ABI_VERSION, YogApi};

/// Inter-mod communication proc-macros.
pub use yog_interop::{yog_export, import};

/// rkyv — zero-copy serialization for interop. Re-exported so generated
/// code and mods can use `rkyv::Archive`, `rkyv::Serialize`, `rkyv::Deserialize`
/// without a direct dependency.
pub use rkyv;

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

            // Catch panics so they never unwind across this `extern "C"` boundary
            // back into the runtime (which would be undefined behaviour).
            let outcome = ::std::panic::catch_unwind(::std::panic::AssertUnwindSafe(|| {
                // SAFETY: the runtime passes a valid YogApi pointer, verified via
                // yog_abi_version() and abi_version/size checks before this call.
                let mut registry = unsafe { $crate::Registry::from_raw(api) };
                <$mod_ty as $crate::Mod>::register(&mut registry);

                // Auto-register all #[yog_export] functions (via linkme distributed slice).
                for entry in $crate::YOG_EXPORTS.iter() {
                    registry.interop().export(entry.name, entry.ptr as *const ::std::os::raw::c_void);
                }

                // Resolve pending imports (may partially succeed — runtime
                // does a final pass after all mods loaded).
                $crate::__yog_resolve_pending_imports(&registry);
            }));
            if outcome.is_err() {
                $crate::error!("mod {} panicked during register", ::core::stringify!($mod_ty));
            }
        }

        /// Called by the runtime after ALL mods are loaded — resolves any
        /// imports that weren't satisfied during initial registration.
        #[no_mangle]
        pub unsafe extern "C" fn __yog_resolve_imports_final(
            api: *const $crate::YogApi,
        ) {
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

std::thread_local! {
    static CURRENT_MOD_ID: std::cell::RefCell<Option<String>> = std::cell::RefCell::new(None);
}

// ── Auto-export registry (used by #[yog_export] proc-macro) ──────────────

/// An entry in the distributed export slice. Each `#[yog_export]` function
/// contributes one entry via `#[linkme::distributed_slice]`.
#[doc(hidden)]
pub struct YogExportEntry {
    pub name: &'static str,
    pub ptr: usize,
}

/// Distributed slice — populated by `#[yog_export]` across all modules.
#[doc(hidden)]
#[linkme::distributed_slice]
pub static YOG_EXPORTS: [YogExportEntry] = [..];

/// An entry in the distributed pending-imports slice.
#[doc(hidden)]
pub struct YogImportEntry {
    pub mod_id: &'static str,
    pub symbol: &'static str,
    pub bind_fn: usize,
}

/// Distributed slice — populated by `import!` across all modules.
#[doc(hidden)]
#[linkme::distributed_slice]
pub static YOG_IMPORTS: [YogImportEntry] = [..];

/// Called by `export_mod!` after registration. Iterates pending imports and
/// resolves them via the interop table. Also exported as `#[no_mangle]` so the
/// runtime can call it after all mods are loaded (final resolution pass).
#[doc(hidden)]
pub unsafe fn __yog_resolve_pending_imports(registry: &crate::Registry) {
    let interop = registry.interop();
    for entry in YOG_IMPORTS.iter() {
        if let Some(ptr) = interop.import_raw(entry.mod_id, entry.symbol) {
            let bind: unsafe extern "C" fn(*const std::ffi::c_void) = std::mem::transmute(entry.bind_fn);
            bind(ptr);
        }
    }
}

pub use yog_command::CommandContext;
pub use yog_core::{BlockPos, Server};
pub use yog_event::{
    AdvancementEvent, AttackEntityEvent, BlockBreakEvent, ChatEvent, ClientTickEvent,
    ContainerCloseEvent, ContainerOpenEvent, CraftEvent, EntityDamageEvent, EntityDeathEvent,
    EntityInteractEvent, EntitySpawnEvent, EventPhase, ExplosionEvent,
    ItemPickupEvent, KeyPressEvent, PlaceBlockEvent, PlayerDeathEvent, PlayerJoinEvent,
    PlayerLeaveEvent, PlayerMoveEvent, PlayerRespawnEvent, ProjectileHitEvent, ScreenEvent,
    UseBlockEvent, UseItemEvent,
};
pub use yog_entity::Entity;
pub use yog_network::{Packet, PacketEvent, PacketField};
#[doc(inline)]
pub use yog_network::packet;
pub use yog_player::Player;
pub use yog_registry::{BlockDef, FoodDef, FurnaceRecipe, ItemDef, ShapedRecipe, ShapelessRecipe, BookRecipe, ItemModifier, AdvancementReward, StartupGrant};
pub use yog_config::Config;
pub use yog_storage::{Storage, StorageScope, Value};
pub use yog_world::World;
pub use yog_book::{Book, BookCategory, BookEntry, BookPage, BookMacro, BookRegistry};
pub use yog_book::{BookRenderer, BookFontRegistry};
pub use yog_book::{text_page, text_page_titled, spotlight_page, crafting_page, smelting_page, image_page, entity_page, relations_page, pattern_page};
pub use yog_ui::{UiRoot, LayoutNode, Rect, widget, Align, FlexDir, Dock, FocusStyle};
pub use yog_inventory::{InventoryDef, SlotLayout};

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
