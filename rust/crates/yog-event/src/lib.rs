//! Event types passed from Minecraft into Rust mods.
//!
//! The subscription hub (`Registry`) lives in the `yog-api` facade, where all
//! domains compose; this crate is just the event vocabulary.

mod events;

pub use events::{
    AdvancementEvent, AttackEntityEvent, BlockBreakEvent, ChatEvent, ContainerCloseEvent,
    ContainerOpenEvent, CraftEvent, EntityDamageEvent, EntityDeathEvent, EntityInteractEvent,
    EntitySpawnEvent, EventPhase, ExplosionEvent, ItemPickupEvent, PlaceBlockEvent,
    PlayerDeathEvent, PlayerJoinEvent, PlayerLeaveEvent, PlayerMoveEvent, PlayerRespawnEvent,
    ProjectileHitEvent, UseBlockEvent, UseItemEvent,
};
