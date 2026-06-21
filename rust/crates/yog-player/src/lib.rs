//! Player access for Yog mods.
//!
//! An ergonomic handle over the player primitives on [`yog_core::Server`]: bind
//! a [`Player`] to a name once, then act on them. Mirrors the shape of
//! `yog-world`'s `World` so the API feels consistent.

use yog_core::Server;

/// A handle to one player by name, bound to a [`Server`].
pub struct Player<'a> {
    server: &'a dyn Server,
    name: String,
}

impl<'a> Player<'a> {
    /// Bind to the player called `name` on `server`.
    pub fn new(server: &'a dyn Server, name: impl Into<String>) -> Self {
        Self {
            server,
            name: name.into(),
        }
    }

    /// The player's name.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Give `count` of `item_id` (e.g. `minecraft:diamond`). Returns whether it
    /// worked (player online, item valid).
    pub fn give(&self, item_id: &str, count: u32) -> bool {
        self.server.give_item(&self.name, item_id, count)
    }

    /// Teleport to `(x, y, z)` in the player's current world.
    pub fn teleport(&self, x: f64, y: f64, z: f64) -> bool {
        self.server.teleport(&self.name, x, y, z)
    }
}
