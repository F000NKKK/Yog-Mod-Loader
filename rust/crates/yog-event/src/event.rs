//! Events on the bus: each event type's addresses, and its payload's bytes.

use yog_data::Schema;
use yog_ids::event::*;

use crate::events::*;

/// An event the bus carries; its payload is `Self` in `yog-data`'s binary form.
pub trait Event: Schema + Sized {
    /// Where it's emitted after the game did it.
    const ADDRESS: &'static str;
    /// Where it's emitted before — a handler's cancel stops it — if it can be stopped.
    const PRE: Option<&'static str>;

    /// The payload's bytes.
    fn to_bytes(&self) -> Vec<u8> {
        yog_data::binary::encode(&self.write())
    }

    /// The event read from a payload; `None` if it isn't one.
    fn from_bytes(bytes: &[u8]) -> Option<Self> {
        yog_data::read_bytes(bytes).ok()
    }
}

macro_rules! events {
    ($($ty:ident => $post:ident, $pre:expr;)*) => {
        $(impl Event for $ty {
            const ADDRESS: &'static str = $post;
            const PRE: Option<&'static str> = $pre;
        })*

        /// Every event type's descriptor: its name, addresses and payload shape.
        pub fn all() -> Vec<(&'static str, &'static str, Option<&'static str>, yog_data::Shape)> {
            vec![$((stringify!($ty), $post, $pre, <$ty as Schema>::shape())),*]
        }
    };
}

events! {
    BlockBreakEvent => BLOCK_BREAK, Some(BLOCK_BREAK_PRE);
    ChatEvent => CHAT, Some(CHAT_PRE);
    PlayerJoinEvent => PLAYER_JOIN, None;
    PlayerLeaveEvent => PLAYER_LEAVE, None;
    UseItemEvent => USE_ITEM, Some(USE_ITEM_PRE);
    UseBlockEvent => USE_BLOCK, Some(USE_BLOCK_PRE);
    AttackEntityEvent => ATTACK_ENTITY, None;
    EntityDamageEvent => ENTITY_DAMAGE, Some(ENTITY_DAMAGE_PRE);
    EntityDeathEvent => ENTITY_DEATH, None;
    PlaceBlockEvent => PLACE_BLOCK, Some(PLACE_BLOCK_PRE);
    EntitySpawnEvent => ENTITY_SPAWN, Some(ENTITY_SPAWN_PRE);
    PlayerDeathEvent => PLAYER_DEATH, Some(PLAYER_DEATH_PRE);
    PlayerRespawnEvent => PLAYER_RESPAWN, None;
    AdvancementEvent => ADVANCEMENT, None;
    EntityInteractEvent => ENTITY_INTERACT, Some(ENTITY_INTERACT_PRE);
    CraftEvent => CRAFT, None;
    ExplosionEvent => EXPLOSION, Some(EXPLOSION_PRE);
    ItemInsertEvent => ITEM_INSERT, None;
    ItemPickupEvent => ITEM_PICKUP, Some(ITEM_PICKUP_PRE);
    PlayerMoveEvent => PLAYER_MOVE, None;
    ContainerOpenEvent => CONTAINER_OPEN, Some(CONTAINER_OPEN_PRE);
    ContainerCloseEvent => CONTAINER_CLOSE, None;
    ClientTickEvent => CLIENT_TICK, None;
    HudRenderEvent => HUD_RENDER, None;
    KeyPressEvent => KEY_PRESS, Some(KEY_PRESS_PRE);
    ProjectileHitEvent => PROJECTILE_HIT, Some(PROJECTILE_HIT_PRE);
    ScreenOpenEvent => SCREEN_OPEN, None;
    ScreenCloseEvent => SCREEN_CLOSE, None;
    ServerTickEvent => SERVER_TICK, None;
    ServerStartedEvent => SERVER_STARTED, None;
    ServerStoppingEvent => SERVER_STOPPING, None;
}
