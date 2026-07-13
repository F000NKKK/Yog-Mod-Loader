//! World access for Yog mods.
//!
//! A thin, ergonomic wrapper over the block primitives on [`yog_core::Server`]:
//! bind a [`World`] to a dimension once, then `get_block` / `set_block` by
//! position. World-domain events will live here too as they land.

use yog_core::{BlockPos, Server};

/// Well-known vanilla dimension ids.
pub mod dimension {
    pub const OVERWORLD: &str = "minecraft:overworld";
    pub const THE_NETHER: &str = "minecraft:the_nether";
    pub const THE_END: &str = "minecraft:the_end";
}

/// An ergonomic handle to one dimension's blocks, bound to a [`Server`].
pub struct World<'a> {
    server: &'a dyn Server,
    dimension: String,
}

impl<'a> World<'a> {
    /// Bind to `dimension` (e.g. [`dimension::OVERWORLD`]) on `server`.
    pub fn new(server: &'a dyn Server, dimension: impl Into<String>) -> Self {
        Self {
            server,
            dimension: dimension.into(),
        }
    }

    /// Registry id of the block at `pos`, or `None` if unavailable.
    pub fn get_block(&self, pos: BlockPos) -> Option<String> {
        self.server.get_block(&self.dimension, pos)
    }

    /// Set the block at `pos` to `block_id`; returns whether it was applied.
    pub fn set_block(&self, pos: BlockPos, block_id: &str) -> bool {
        self.server.set_block(&self.dimension, pos, block_id)
    }

    /// Game time in ticks since world creation.
    pub fn time(&self) -> Option<i64> {
        self.server.world_time(&self.dimension)
    }

    /// Set the time-of-day (0 = dawn, 6000 = noon, 12000 = dusk, 18000 = midnight).
    pub fn set_time(&self, time: i64) -> bool {
        self.server.world_set_time(&self.dimension, time)
    }

    /// Whether it is currently raining in this dimension.
    pub fn is_raining(&self) -> bool {
        self.server.world_is_raining(&self.dimension)
    }

    /// Start or stop rain. `duration_ticks = 0` picks a server-default duration.
    pub fn set_weather(&self, raining: bool, duration_ticks: i32) -> bool {
        self.server
            .world_set_weather(&self.dimension, raining, duration_ticks)
    }

    /// Number of loaded entities of `entity_type` (e.g. `"minecraft:zombie"`)
    /// in this dimension. Returns `-1` if the dimension or type is unknown.
    pub fn entity_count(&self, entity_type: &str) -> i32 {
        self.server.world_entity_count(&self.dimension, entity_type)
    }
}

/// Convenience: the overworld of `server`.
pub fn overworld(server: &dyn Server) -> World<'_> {
    World::new(server, dimension::OVERWORLD)
}
