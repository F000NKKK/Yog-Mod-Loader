//! Entity access — a universal handle to *any* entity by UUID.
//!
//! In Minecraft most actions are entity-level (Player → LivingEntity → Entity):
//! teleport, position, health... So Yog exposes them here, by UUID, for any
//! entity. `Player` (in `yog-player`) is a thin wrapper that adds player-only
//! things (inventory, networking) on top.

use yog_core::Server;

/// A handle to one entity by UUID, bound to a [`Server`].
pub struct Entity<'a> {
    server: &'a dyn Server,
    uuid: String,
}

impl<'a> Entity<'a> {
    /// Bind to the entity with this UUID on `server`.
    pub fn new(server: &'a dyn Server, uuid: impl Into<String>) -> Self {
        Self {
            server,
            uuid: uuid.into(),
        }
    }

    pub fn uuid(&self) -> &str {
        &self.uuid
    }

    /// Teleport to `(x, y, z)` within the entity's current world.
    pub fn teleport(&self, x: f64, y: f64, z: f64) -> bool {
        self.server.entity_teleport(&self.uuid, x, y, z)
    }

    /// Current position, or `None` if the entity isn't loaded.
    pub fn position(&self) -> Option<(f64, f64, f64)> {
        self.server.entity_position(&self.uuid)
    }

    /// Health (living entities only), or `None`.
    pub fn health(&self) -> Option<f32> {
        self.server.entity_health(&self.uuid)
    }

    /// Set health (living entities only); returns whether it applied.
    pub fn set_health(&self, health: f32) -> bool {
        self.server.entity_set_health(&self.uuid, health)
    }

    /// Remove/kill the entity.
    pub fn kill(&self) -> bool {
        self.server.entity_kill(&self.uuid)
    }
}
