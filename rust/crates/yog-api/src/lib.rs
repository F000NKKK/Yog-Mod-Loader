//! Yog API — the single crate mod authors depend on.
//!
//! A facade that re-exports every Yog domain plus the central [`Registry`] hub.
//! Add a new domain crate, re-export it here, and mods pick it up via
//! `yog_api::*`. Items are available both flat (`yog_api::Registry`) and
//! namespaced by domain (`yog_api::world::World`).

mod registry;

pub use registry::{Mod, Registry};

pub use yog_command::CommandContext;
pub use yog_core::{BlockPos, Server};
pub use yog_event::{BlockBreakEvent, ChatEvent, PlayerJoinEvent, PlayerLeaveEvent};
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

/// Commands.
pub mod command {
    pub use yog_command::*;
}
