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

    // ── status effects ──────────────────────────────────────────────────────

    /// Apply a status effect. `effect_id` is a registry id like
    /// `"minecraft:speed"`. `amplifier` is 0-based (0 = level I).
    pub fn add_effect(
        &self,
        effect_id: &str,
        duration_ticks: i32,
        amplifier: u8,
        show_particles: bool,
    ) -> bool {
        self.server.entity_add_effect(&self.uuid, effect_id, duration_ticks, amplifier, show_particles)
    }

    /// Remove a single status effect.
    pub fn remove_effect(&self, effect_id: &str) -> bool {
        self.server.entity_remove_effect(&self.uuid, effect_id)
    }

    /// Clear all active status effects.
    pub fn clear_effects(&self) -> bool {
        self.server.entity_clear_effects(&self.uuid)
    }

    // ── velocity ────────────────────────────────────────────────────────────

    /// Current velocity `(vx, vy, vz)`, or `None` if not loaded.
    pub fn velocity(&self) -> Option<(f64, f64, f64)> {
        self.server.entity_velocity(&self.uuid)
    }

    /// Set velocity directly (replaces current velocity).
    pub fn set_velocity(&self, vx: f64, vy: f64, vz: f64) -> bool {
        self.server.entity_set_velocity(&self.uuid, vx, vy, vz)
    }

    /// Add an impulse to the current velocity (e.g. launch upward: `add_velocity(0, 1, 0)`).
    pub fn add_velocity(&self, vx: f64, vy: f64, vz: f64) -> bool {
        self.server.entity_add_velocity(&self.uuid, vx, vy, vz)
    }

    // ── NBT ─────────────────────────────────────────────────────────────────

    /// SNBT string of the entity's persistent NBT, or `None` if not found.
    pub fn get_nbt(&self) -> Option<String> {
        self.server.entity_get_nbt(&self.uuid)
    }

    /// Merge SNBT data into the entity's persistent NBT. Returns `false` if not found.
    pub fn set_nbt(&self, snbt: &str) -> bool {
        self.server.entity_set_nbt(&self.uuid, snbt)
    }

    // ── attributes ───────────────────────────────────────────────────────────

    /// Base value of an attribute (e.g. `"minecraft:generic.max_health"`), or `None`.
    pub fn attribute_get(&self, attribute_id: &str) -> Option<f64> {
        self.server.entity_attribute_get(&self.uuid, attribute_id)
    }

    /// Set the base value of an attribute. Returns `false` if not found.
    pub fn attribute_set(&self, attribute_id: &str, value: f64) -> bool {
        self.server.entity_attribute_set(&self.uuid, attribute_id, value)
    }
}
