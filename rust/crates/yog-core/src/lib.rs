//! Core Yog types and handles shared across all API modules.
//!
//! Kept tiny and dependency-free: every other `yog-*` crate builds on this, and
//! the facade [`yog-api`] re-exports it.

mod server;

pub use server::Server;

/// A block position in the world.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BlockPos {
    pub x: i32,
    pub y: i32,
    pub z: i32,
}

impl BlockPos {
    pub const fn new(x: i32, y: i32, z: i32) -> Self {
        Self { x, y, z }
    }
}
