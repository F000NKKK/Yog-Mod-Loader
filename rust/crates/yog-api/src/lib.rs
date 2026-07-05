//! Yog API — the single crate mod authors depend on.
//!
//! A facade that re-exports every Yog domain plus the central [`Registry`] hub.
//! Add a new domain crate, re-export it here, and mods pick it up via
//! `yog_api::*`. Items are available both flat (`yog_api::Registry`) and
//! namespaced by domain (`yog_api::world::World`).

mod registry;

pub use registry::{installed_mods, open_ui, server, CServer, Mod, ModInfo, Registry};
pub use yog_gfx::{GfxContext, core as gfx_core, gl as gfx_gl, draw2d as gfx_draw2d};

/// Stable C ABI — re-exported so mods don't need a direct `yog-abi` dependency.
pub use yog_abi::{ABI_VERSION, YogApi};

#[doc(hidden)]
pub use std::os::raw::c_void as __c_void;

/// Export a [`Mod`] as a dynamically loadable Yog mod.
///
/// Generates the two C-ABI entry points the runtime looks up:
/// - `yog_abi_version() -> u32`  — version check before loading
/// - `yog_mod_register(*const YogApi, *mut c_void)` — registration entry point
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
            _ctx: *mut $crate::__c_void,
        ) {
            // Catch panics so they never unwind across this `extern "C"` boundary
            // back into the runtime (which would be undefined behaviour).
            let outcome = ::std::panic::catch_unwind(::std::panic::AssertUnwindSafe(|| {
                // SAFETY: the runtime passes a valid YogApi pointer, verified via
                // yog_abi_version() and abi_version/size checks before this call.
                let mut registry = unsafe { $crate::Registry::from_raw(api) };
                <$mod_ty as $crate::Mod>::register(&mut registry);
            }));
            if outcome.is_err() {
                $crate::error!("mod {} panicked during register", ::core::stringify!($mod_ty));
            }
        }
    };
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
