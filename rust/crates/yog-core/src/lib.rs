//! Core Yog types and handles shared across all API modules.
//!
//! Kept tiny — Yog Data, so its records travel the bus, and mirrors'
//! cells, so its positions are kept in them: every other `yog-*` crate builds on this, and the facade
//! [`yog-api`] re-exports it.

mod cells;
mod local;
mod query;
mod state;

pub use local::{BlockArea, BlockKind, ClickMode, LocalState, NearbyEntity};
pub use query::{
    BlockHit, BlockInfo, EntityInfo, Face, PlacedBlock, PlayerInfo, Rotation, ScreenSlot,
    SlotStack, Stack, entity_flags,
};
pub use state::{join_state, split_state};

/// A block position in the world.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, yog_data::Schema)]
#[data(view)]
pub struct BlockPos {
    pub x: i32,
    pub y: i32,
    pub z: i32,
}

/// A point in a world (blocks, fractional).
#[derive(Debug, Clone, Copy, PartialEq, Default, yog_data::Schema)]
#[data(view)]
pub struct Point {
    pub x: f64,
    pub y: f64,
    pub z: f64,
}

impl BlockPos {
    pub const fn new(x: i32, y: i32, z: i32) -> Self {
        Self { x, y, z }
    }
}

impl From<(i32, i32, i32)> for BlockPos {
    fn from((x, y, z): (i32, i32, i32)) -> Self {
        Self { x, y, z }
    }
}

/// `[x, y, z]`, as math on positions writes them.
impl From<[i32; 3]> for BlockPos {
    fn from([x, y, z]: [i32; 3]) -> Self {
        Self { x, y, z }
    }
}

impl From<BlockPos> for [i32; 3] {
    fn from(p: BlockPos) -> Self {
        [p.x, p.y, p.z]
    }
}

impl From<[f64; 3]> for Point {
    fn from([x, y, z]: [f64; 3]) -> Self {
        Self { x, y, z }
    }
}

impl From<Point> for [f64; 3] {
    fn from(p: Point) -> Self {
        [p.x, p.y, p.z]
    }
}

impl From<(f64, f64, f64)> for Point {
    fn from((x, y, z): (f64, f64, f64)) -> Self {
        Self { x, y, z }
    }
}

impl From<BlockPos> for Point {
    /// The block's corner (its lowest x, y, z).
    fn from(p: BlockPos) -> Self {
        Self {
            x: f64::from(p.x),
            y: f64::from(p.y),
            z: f64::from(p.z),
        }
    }
}
