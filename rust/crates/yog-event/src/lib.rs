//! The bus's vocabulary: the events Minecraft fires (through the Java
//! host), by plane — `player::session::Join`, `block::Break`,
//! `server::game::Tick` … — and the services someone provides
//! (`block::Insert`), each a message with its address and its payload's
//! schema.
//!
//! Subscribing and providing (`Registry::on`/`provide`…) live in the
//! `yog-api` facade.

#[macro_use]
mod message;
mod address;
mod arg;
mod call;
mod event;
mod handles;
pub mod java;
mod phase;
mod service;
#[cfg(test)]
mod tests;

pub mod advancement;
pub mod block;
pub mod bossbar;
pub mod bus;
pub mod chat;
pub mod client;
pub mod command;
pub mod container;
pub mod content;
pub mod entity;
pub mod game;
pub mod item;
pub mod jvm;
pub mod key_binding;
pub mod log;
pub mod media;
pub mod menu;
pub mod mirror;
pub mod nanite;
pub mod net;
pub mod particle;
pub mod player;
pub mod scoreboard;
pub mod server;
pub mod sound;
pub mod tag;
pub mod ui;
pub mod video;
pub mod world;

pub use address::{Address, Cached, NO_ADDRESS, resolve};
pub use arg::Arg;
pub use call::{Caller, call, set_caller};
pub use event::{Event, EventInfo, all};
pub use handles::{Entity, Player, World};
pub use message::{Message, keyed};
pub use phase::EventPhase;
pub use service::{Service, ServiceInfo, services};
pub use yog_core::Point;
