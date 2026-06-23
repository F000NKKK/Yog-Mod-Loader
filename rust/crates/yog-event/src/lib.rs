//! Event types passed from Minecraft into Rust mods.
//!
//! The subscription hub (`Registry`) lives in the `yog-api` facade, where all
//! domains compose; this crate is just the event vocabulary.

mod events;

pub use events::{
    AdvancementEvent, AttackEntityEvent, BlockBreakEvent, ChatEvent, ClientTickEvent,
    ContainerCloseEvent, ContainerOpenEvent, CraftEvent, EntityDamageEvent, EntityDeathEvent,
    EntityInteractEvent, EntitySpawnEvent, EventPhase, ExplosionEvent, HudRenderEvent,
    ItemPickupEvent, KeyPressEvent, PlaceBlockEvent, PlayerDeathEvent, PlayerJoinEvent,
    PlayerLeaveEvent, PlayerMoveEvent, PlayerRespawnEvent, ProjectileHitEvent, ScreenEvent,
    UseBlockEvent, UseItemEvent,
};
