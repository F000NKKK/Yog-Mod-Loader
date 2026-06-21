//! Yog API — write Minecraft mods in Rust.
//!
//! "The Gate and the Key": this crate is the surface that mod authors code
//! against. It is pure Rust and has no knowledge of the JVM — the [`yog-runtime`]
//! library is what bridges these events to and from the Java host.

mod events;
mod registry;

pub use events::{BlockBreakEvent, BlockPos, ChatEvent};
pub use registry::{Mod, Registry};
