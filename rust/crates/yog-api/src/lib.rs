//! Yog API — the single crate mod authors depend on.
//!
//! A thin **facade** that re-exports every Yog domain module. Add a new domain
//! crate and re-export it here; mods pick it up automatically via `yog_api::*`.
//!
//! Items are available both flat (`yog_api::Registry`) and namespaced by domain
//! (`yog_api::event::Registry`).

pub use yog_core::{BlockPos, Server};
pub use yog_event::{BlockBreakEvent, ChatEvent, Mod, PlayerJoinEvent, PlayerLeaveEvent, Registry};

/// Core types and handles.
pub mod core {
    pub use yog_core::*;
}

/// Events and the subscription registry.
pub mod event {
    pub use yog_event::*;
}
