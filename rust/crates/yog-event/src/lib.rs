//! The events Minecraft fires (through the Java host) to Rust mods, by
//! plane — `player::Join`, `block::Break`, `server::Tick` … — each a bus
//! message with its addresses and its payload's schema.
//!
//! The subscription hub (`Registry::on`/`before`/`after`) lives in the
//! `yog-api` facade; this crate is the event vocabulary.

mod event;
pub mod java;
mod phase;
mod pos;
#[cfg(test)]
mod tests;

pub mod block;
pub mod client;
pub mod container;
pub mod entity;
pub mod player;
pub mod server;
pub mod world;

pub use event::{Event, EventInfo, all, keyed};
pub use phase::EventPhase;
