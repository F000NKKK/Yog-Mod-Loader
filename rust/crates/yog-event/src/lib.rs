//! Event types passed from Minecraft into Rust mods.
//!
//! The subscription hub (`Registry`) lives in the `yog-api` facade, where all
//! domains compose; this crate is just the event vocabulary.

mod events;

pub use events::{
    AttackEntityEvent, BlockBreakEvent, ChatEvent, EntityDamageEvent, EntityDeathEvent,
    PlayerJoinEvent, PlayerLeaveEvent, UseBlockEvent, UseItemEvent,
};
