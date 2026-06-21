//! Event types and the subscription [`Registry`] mod authors use.

mod events;
mod registry;

pub use events::{BlockBreakEvent, ChatEvent, PlayerJoinEvent, PlayerLeaveEvent};
pub use registry::{Mod, Registry};
