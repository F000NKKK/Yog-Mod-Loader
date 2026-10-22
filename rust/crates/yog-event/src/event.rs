//! Events on the bus: each event type's addresses, and its payload's bytes.

use yog_data::Schema;
use yog_ids as ids;

use crate::message::{Message, branch, last_segment};

use crate::{block, client, container, entity, log, player, server, world};

/// An event the bus carries; its payload is `Self` in `yog-data`'s binary form.
pub trait Event: Message {
    /// Where it's emitted after the game did it.
    const ADDRESS: &'static str;
    /// Where it's emitted before — a handler's cancel stops it — if it can be stopped.
    const PRE: Option<&'static str>;
    /// The field whose value picks the event's branch (`entity_type`: each
    /// type's ticks go to `…/tick/<type>`), if it's keyed — see [`crate::keyed`].
    const KEY: Option<&'static str>;

    /// The key field's value, if it's keyed.
    fn key(&self) -> Option<&str>;

    /// Where this event goes after the game did it: the address, or its branch for the key.
    fn target(&self) -> String {
        crate::message::target(Self::ADDRESS, self.key())
    }

    /// Where it goes before, if it can be stopped.
    fn pre_target(&self) -> Option<String> {
        Self::PRE.map(|pre| crate::message::target(pre, self.key()))
    }

    /// [`Event::target`]'s number — the type's own address cached.
    fn address(&self) -> crate::Address;

    /// [`Event::pre_target`]'s number.
    fn pre_address(&self) -> Option<crate::Address>;
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

            fn key(&self) -> Option<&str> {
                key_value!(self, $($key)?)
            }

            fn address(&self) -> crate::Address {
                static AT: crate::Cached = crate::Cached::new();
                crate::address::of(&AT, $post, self.key())
            }

            fn pre_address(&self) -> Option<crate::Address> {
                static PRE: crate::Cached = crate::Cached::new();
                $pre.map(|pre: &'static str| crate::address::of(&PRE, pre, self.key()))
            }
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
    log::Record => ids::event_log::RECORD, None;
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
    world::generation::Chunk => ids::event_world_generation::CHUNK, None, key = generator;
    world::loot::Roll => ids::event_world_loot::ROLL, None;
    container::Open => ids::event_container::OPEN, Some(ids::event_container::OPEN_PRE);
    container::Close => ids::event_container::CLOSE, None;
    container::machine::Craft => ids::event_container_machine::CRAFT, Some(ids::event_container_machine::CRAFT_PRE);
    server::game::Tick => ids::event_server_game::TICK, None;
    server::life::Started => ids::event_server_life::STARTED, None;
    server::life::Stopping => ids::event_server_life::STOPPING, None;
    server::life::Saving => ids::event_server_life::SAVING, None;
    server::command::Run => ids::event_server_command::RUN, None, key = name;
    server::net::Packet => ids::event_server_net::PACKET, None, key = channel;
    client::game::Tick => ids::event_client_game::TICK, None;
    client::game::Travel => ids::event_client_game::TRAVEL, None;
    client::input::Key => ids::event_client_input::KEY, Some(ids::event_client_input::KEY_PRE);
    client::input::Binding => ids::event_client_input::BINDING, None, key = id;
    client::ui::Open => ids::event_client_ui::OPEN, None;
    client::ui::Close => ids::event_client_ui::CLOSE, None;
    client::ui::Input => ids::event_client_ui::INPUT, Some(ids::event_client_ui::INPUT_PRE), key = ui_id;
    client::render::Hud => ids::event_client_render::HUD, None;
    client::render::World => ids::event_client_render::WORLD, None;
    client::render::Ui => ids::event_client_render::UI, None, key = ui_id;
    client::render::Tooltip => ids::event_client_render::TOOLTIP, None, key = item;
    client::render::Page => ids::event_client_render::PAGE, None, key = drawer;
    client::render::Camera => ids::event_client_render::CAMERA, None;
    client::render::Body => ids::event_client_render::BODY, None;
    client::net::Packet => ids::event_client_net::PACKET, None, key = channel;
}
