//! Events on the bus: each event type's addresses, and its payload's bytes.

use yog_data::Schema;
use yog_ids as ids;

use crate::message::{Message, branch, last_segment};

use crate::{block, client, container, entity, player, server, world};

/// An event the bus carries; its payload is `Self` in `yog-data`'s binary form.
pub trait Event: Message {
    /// Where it's emitted after the game did it.
    const ADDRESS: &'static str;
    /// Where it's emitted before — a handler's cancel stops it — if it can be stopped.
    const PRE: Option<&'static str>;
    /// The field whose value picks the event's branch (`entity_type`: each
    /// type's ticks go to `…/tick/<type>`), if it's keyed — see [`crate::keyed`].
    const KEY: Option<&'static str>;
}

/// An event type's descriptor: its plane and name, addresses and payload shape.
pub struct EventInfo {
    /// Its place in the tree: `player/session`, `block`, `client/ui`…
    pub plane: &'static str,
    pub name: &'static str,
    pub address: &'static str,
    pub pre: Option<&'static str>,
    pub key: Option<&'static str>,
    pub shape: yog_data::Shape,
}

macro_rules! events {
    ($($ty:path => $post:expr, $pre:expr $(, key = $key:ident)?;)*) => {
        $(impl Event for $ty {
            const ADDRESS: &'static str = $post;
            const PRE: Option<&'static str> = $pre;
            const KEY: Option<&'static str> = key!($($key)?);
        })*

        /// Every event type, plane by plane.
        pub fn all() -> Vec<EventInfo> {
            vec![$(EventInfo {
                plane: branch($post, ids::event::PREFIX),
                name: last_segment(stringify!($ty)),
                address: $post,
                pre: $pre,
                key: key!($($key)?),
                shape: <$ty as Schema>::shape(),
            }),*]
        }
    };
}

events! {
    player::session::Join => ids::event_player_session::JOIN, None;
    player::session::Leave => ids::event_player_session::LEAVE, None;
    player::life::Death => ids::event_player_life::DEATH, Some(ids::event_player_life::DEATH_PRE);
    player::life::Respawn => ids::event_player_life::RESPAWN, None;
    player::chat::Message => ids::event_player_chat::MESSAGE, Some(ids::event_player_chat::MESSAGE_PRE);
    player::action::Move => ids::event_player_action::MOVE, None;
    player::action::UseItem => ids::event_player_action::USE_ITEM, Some(ids::event_player_action::USE_ITEM_PRE);
    player::action::Attack => ids::event_player_action::ATTACK, None;
    player::action::Interact => ids::event_player_action::INTERACT, Some(ids::event_player_action::INTERACT_PRE);
    player::action::Craft => ids::event_player_action::CRAFT, None;
    player::action::Pickup => ids::event_player_action::PICKUP, Some(ids::event_player_action::PICKUP_PRE);
    player::progress::Advancement => ids::event_player_progress::ADVANCEMENT, None;
    block::Break => ids::event_block::BREAK, Some(ids::event_block::BREAK_PRE);
    block::Place => ids::event_block::PLACE, Some(ids::event_block::PLACE_PRE);
    block::Use => ids::event_block::USE, Some(ids::event_block::USE_PRE);
    entity::life::Spawn => ids::event_entity_life::SPAWN, Some(ids::event_entity_life::SPAWN_PRE);
    entity::life::Death => ids::event_entity_life::DEATH, None;
    entity::combat::Damage => ids::event_entity_combat::DAMAGE, Some(ids::event_entity_combat::DAMAGE_PRE);
    entity::combat::ProjectileHit => ids::event_entity_combat::PROJECTILE_HIT, Some(ids::event_entity_combat::PROJECTILE_HIT_PRE);
    entity::game::Tick => ids::event_entity_game::TICK, None, key = entity_type;
    world::Explosion => ids::event_world::EXPLOSION, Some(ids::event_world::EXPLOSION_PRE);
    world::loot::Roll => ids::event_world_loot::ROLL, None;
    container::Open => ids::event_container::OPEN, Some(ids::event_container::OPEN_PRE);
    container::Close => ids::event_container::CLOSE, None;
    container::machine::Craft => ids::event_container_machine::CRAFT, Some(ids::event_container_machine::CRAFT_PRE);
    server::game::Tick => ids::event_server_game::TICK, None;
    server::life::Started => ids::event_server_life::STARTED, None;
    server::life::Stopping => ids::event_server_life::STOPPING, None;
    server::command::Run => ids::event_server_command::RUN, None, key = name;
    server::net::Packet => ids::event_server_net::PACKET, None, key = channel;
    client::game::Tick => ids::event_client_game::TICK, None;
    client::input::Key => ids::event_client_input::KEY, Some(ids::event_client_input::KEY_PRE);
    client::input::Binding => ids::event_client_input::BINDING, None, key = id;
    client::ui::Open => ids::event_client_ui::OPEN, None;
    client::ui::Close => ids::event_client_ui::CLOSE, None;
    client::render::Hud => ids::event_client_render::HUD, None;
    client::net::Packet => ids::event_client_net::PACKET, None, key = channel;
}
