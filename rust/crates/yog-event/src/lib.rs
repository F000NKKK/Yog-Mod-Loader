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
mod call;
mod event;
mod handles;
mod service;
pub mod java;
mod phase;
#[cfg(test)]
mod tests;

pub mod advancement;
pub mod block;
pub mod bossbar;
pub mod bus;
pub mod chat;
pub mod client;
pub mod command;
pub mod content;
pub mod container;
pub mod entity;
pub mod game;
pub mod item;
pub mod jvm;
pub mod key_binding;
pub mod log;
pub mod menu;
pub mod net;
pub mod particle;
pub mod player;
pub mod scoreboard;
pub mod server;
pub mod sound;
pub mod tag;
pub mod ui;
pub mod world;

pub use address::{Address, Cached, NO_ADDRESS, resolve};
pub use call::{Caller, call, set_caller};
pub use handles::{Entity, Player, World};
pub use event::{Event, EventInfo, all};
pub use message::{Message, keyed};
pub use service::{Service, ServiceInfo, services};
pub use phase::EventPhase;
pub use yog_core::Point;
