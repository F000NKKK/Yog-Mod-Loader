//! Events on the bus: each event type's addresses, and its payload's bytes.

use yog_data::Schema;
use yog_ids as ids;

use crate::{block, client, container, entity, player, server, world};

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

/// An event type's descriptor: its plane and name, addresses and payload shape.
pub struct EventInfo {
    /// Its place in the tree: `player/session`, `block`, `client/ui`…
    pub plane: &'static str,
    pub name: &'static str,
    pub address: &'static str,
    pub pre: Option<&'static str>,
    pub shape: yog_data::Shape,
}

macro_rules! events {
    ($($ty:path, $plane:literal => $post:expr, $pre:expr;)*) => {
        $(impl Event for $ty {
            const ADDRESS: &'static str = $post;
            const PRE: Option<&'static str> = $pre;
        })*

        /// Every event type, plane by plane.
        pub fn all() -> Vec<EventInfo> {
            vec![$(EventInfo {
                plane: $plane,
                name: { let p = stringify!($ty); match p.rfind(':') { Some(i) => p.split_at(i + 1).1.trim(), None => p } },
                address: $post,
                pre: $pre,
                shape: <$ty as Schema>::shape(),
            }),*]
        }
    };
}

events! {
    player::session::Join, "player/session" => ids::event_player_session::JOIN, None;
    player::session::Leave, "player/session" => ids::event_player_session::LEAVE, None;
    player::life::Death, "player/life" => ids::event_player_life::DEATH, Some(ids::event_player_life::DEATH_PRE);
    player::life::Respawn, "player/life" => ids::event_player_life::RESPAWN, None;
    player::chat::Message, "player/chat" => ids::event_player_chat::MESSAGE, Some(ids::event_player_chat::MESSAGE_PRE);
    player::action::Move, "player/action" => ids::event_player_action::MOVE, None;
    player::action::UseItem, "player/action" => ids::event_player_action::USE_ITEM, Some(ids::event_player_action::USE_ITEM_PRE);
    player::action::Attack, "player/action" => ids::event_player_action::ATTACK, None;
    player::action::Interact, "player/action" => ids::event_player_action::INTERACT, Some(ids::event_player_action::INTERACT_PRE);
    player::action::Craft, "player/action" => ids::event_player_action::CRAFT, None;
    player::action::Pickup, "player/action" => ids::event_player_action::PICKUP, Some(ids::event_player_action::PICKUP_PRE);
    player::progress::Advancement, "player/progress" => ids::event_player_progress::ADVANCEMENT, None;
    block::Break, "block" => ids::event_block::BREAK, Some(ids::event_block::BREAK_PRE);
    block::Place, "block" => ids::event_block::PLACE, Some(ids::event_block::PLACE_PRE);
    block::Use, "block" => ids::event_block::USE, Some(ids::event_block::USE_PRE);
    block::Insert, "block" => ids::event_block::INSERT, None;
    entity::life::Spawn, "entity/life" => ids::event_entity_life::SPAWN, Some(ids::event_entity_life::SPAWN_PRE);
    entity::life::Death, "entity/life" => ids::event_entity_life::DEATH, None;
    entity::combat::Damage, "entity/combat" => ids::event_entity_combat::DAMAGE, Some(ids::event_entity_combat::DAMAGE_PRE);
    entity::combat::ProjectileHit, "entity/combat" => ids::event_entity_combat::PROJECTILE_HIT, Some(ids::event_entity_combat::PROJECTILE_HIT_PRE);
    world::Explosion, "world" => ids::event_world::EXPLOSION, Some(ids::event_world::EXPLOSION_PRE);
    container::Open, "container" => ids::event_container::OPEN, Some(ids::event_container::OPEN_PRE);
    container::Close, "container" => ids::event_container::CLOSE, None;
    server::game::Tick, "server/game" => ids::event_server_game::TICK, None;
    server::life::Started, "server/life" => ids::event_server_life::STARTED, None;
    server::life::Stopping, "server/life" => ids::event_server_life::STOPPING, None;
    client::game::Tick, "client/game" => ids::event_client_game::TICK, None;
    client::input::Key, "client/input" => ids::event_client_input::KEY, Some(ids::event_client_input::KEY_PRE);
    client::ui::Open, "client/ui" => ids::event_client_ui::OPEN, None;
    client::ui::Close, "client/ui" => ids::event_client_ui::CLOSE, None;
    client::render::Hud, "client/render" => ids::event_client_render::HUD, None;
}
