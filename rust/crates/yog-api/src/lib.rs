//! Yog API — the single crate mod authors depend on.
//!
//! A facade that re-exports every Yog domain plus the central [`Registry`] hub.
//! Add a new domain crate, re-export it here, and mods pick it up via
//! `yog_api::*`. Items are available both flat (`yog_api::Registry`) and
//! namespaced by domain (`yog_api::world::World`).

mod registry;

pub use registry::{Mod, Registry};

/// ABI version of the dynamic-mod interface. The runtime refuses to load a mod
/// whose `yog_abi_version()` does not match, since Rust has no stable ABI:
/// runtime and mods must be built against the same `yog-api`.
pub const ABI_VERSION: u32 = 1;

/// Export a [`Mod`] as a dynamically loadable Yog mod.
///
/// Generates the C-ABI entry points the runtime looks up (`yog_abi_version`,
/// `yog_mod_register`) so mod authors never write `unsafe`. Put this once at the
/// crate root of a `cdylib` mod:
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
        pub extern "C" fn yog_mod_register(registry: *mut $crate::Registry) {
            // Catch panics so they never unwind across this `extern "C"` boundary
            // back into the runtime (which would be undefined behaviour).
            let outcome = ::std::panic::catch_unwind(::std::panic::AssertUnwindSafe(|| {
                // SAFETY: the runtime passes a valid, exclusive pointer to a
                // Registry built against the same yog-api version, verified via
                // yog_abi_version() before this call.
                let registry: &mut $crate::Registry = unsafe { &mut *registry };
                <$mod_ty as $crate::Mod>::register(registry);
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
    AttackEntityEvent, BlockBreakEvent, ChatEvent, EntityDamageEvent, EntityDeathEvent,
    PlayerJoinEvent, PlayerLeaveEvent, UseBlockEvent, UseItemEvent,
};
pub use yog_entity::Entity;
pub use yog_network::PacketEvent;
pub use yog_player::Player;
pub use yog_registry::{BlockDef, FoodDef, ItemDef};
pub use yog_storage::Storage;
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
