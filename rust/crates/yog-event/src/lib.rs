//! The bus's vocabulary: the events Minecraft fires (through the Java
//! host), by plane — `player::session::Join`, `block::Break`,
//! `server::game::Tick` … — and the services someone provides
//! (`block::Insert`), each a message with its address and its payload's
//! schema.
//!
//! Subscribing and providing (`Registry::on`/`provide`…) live in the
//! `yog-api` facade.

#[macro_use]
mod wire;
mod planes;

pub use planes::living::{entity, player};
pub use planes::play::{advancement, bossbar, chat, command, game, scoreboard, server};
pub use planes::system::{bus, java, jvm, log, mirror, nanite, net, storage};
pub use planes::things::{block, container, content, item, particle, sound, tag, world};
pub use planes::view::{client, key_binding, media, menu, ui, video};

pub use wire::address::{Address, Cached, NO_ADDRESS, resolve};
pub use wire::arg::Arg;
pub use wire::call::{Caller, call, set_caller};
pub use wire::event::{Event, EventInfo, all};
pub use wire::handles::{Entity, Player, SUBJECTS, World};
pub use wire::message::{Message, keyed};
pub use wire::phase::EventPhase;
pub use wire::service::{Service, ServiceInfo, services};
pub use yog_core::Point;
