//! Player access for Yog mods.
//!
//! [`Player`] is a thin wrapper over [`yog_entity::Entity`]: it delegates all
//! entity-level operations (teleport, position, health …) to that layer and
//! adds the player-specific extras (inventory, networking) on top.

use yog_core::Server;
use yog_entity::Entity;

/// A handle to one player, bound to a [`Server`].
///
/// Construct with [`Player::new`] when you only have the player name (most
/// event callbacks), or with [`Player::with_uuid`] when you also have the UUID
/// (e.g. from [`yog_command::CommandContext`]) — the latter unlocks the full
/// entity-layer API.
pub struct Player<'a> {
    server: &'a dyn Server,
    name: String,
    uuid: Option<String>,
}

impl<'a> Player<'a> {
    /// Bind to the player called `name`. Entity-level ops that require a UUID
    /// will return `None`/`false`; use [`Player::with_uuid`] to unlock them.
    pub fn new(server: &'a dyn Server, name: impl Into<String>) -> Self {
        Self { server, name: name.into(), uuid: None }
    }

    /// Bind to the player with both `name` and `uuid` (full functionality).
    pub fn with_uuid(
        server: &'a dyn Server,
        name: impl Into<String>,
        uuid: impl Into<String>,
    ) -> Self {
        Self { server, name: name.into(), uuid: Some(uuid.into()) }
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn uuid(&self) -> Option<&str> {
        self.uuid.as_deref()
    }

    /// The underlying [`Entity`] handle, available when a UUID was provided.
    pub fn entity(&self) -> Option<Entity<'_>> {
        self.uuid.as_deref().map(|u| Entity::new(self.server, u))
    }

    // ── player-specific ops ─────────────────────────────────────────────────

    /// Give `count` of `item_id` (e.g. `"minecraft:diamond"`).
    pub fn give(&self, item_id: &str, count: u32) -> bool {
        self.server.give_item(&self.name, item_id, count)
    }

    /// Send a raw-byte packet to this player on `channel` (server → client).
    pub fn send_packet(&self, channel: &str, payload: &[u8]) -> bool {
        self.server.send_to_player(&self.name, channel, payload)
    }

    // ── entity-level ops (delegated) ────────────────────────────────────────

    /// Teleport to `(x, y, z)`. Uses the entity layer when a UUID is known;
    /// falls back to the player-name primitive otherwise.
    pub fn teleport(&self, x: f64, y: f64, z: f64) -> bool {
        match &self.uuid {
            Some(u) => self.server.entity_teleport(u, x, y, z),
            None => self.server.teleport(&self.name, x, y, z),
        }
    }

    /// Current position, or `None` if the entity isn't loaded / UUID unknown.
    pub fn position(&self) -> Option<(f64, f64, f64)> {
        self.entity()?.position()
    }

    /// Health, or `None` if UUID unknown.
    pub fn health(&self) -> Option<f32> {
        self.entity()?.health()
    }

    /// Set health; returns `false` if UUID unknown.
    pub fn set_health(&self, health: f32) -> bool {
        self.entity().map_or(false, |e: Entity<'_>| e.set_health(health))
    }

    /// Kill/remove this player entity; returns `false` if UUID unknown.
    pub fn kill(&self) -> bool {
        self.entity().map_or(false, |e: Entity<'_>| e.kill())
    }

    /// Send a title+subtitle screen to this player.
    pub fn send_title(
        &self,
        title: &str,
        subtitle: &str,
        fadein: i32,
        stay: i32,
        fadeout: i32,
    ) -> bool {
        self.server.send_title(&self.name, title, subtitle, fadein, stay, fadeout)
    }

    /// Send a message to the action-bar (above hotbar).
    pub fn send_actionbar(&self, message: &str) -> bool {
        self.server.send_actionbar(&self.name, message)
    }

    /// Disconnect this player with a reason message.
    pub fn kick(&self, reason: &str) -> bool {
        self.server.kick_player(&self.name, reason)
    }

    /// Change this player's game mode (`"survival"`, `"creative"`, `"adventure"`, `"spectator"`).
    pub fn set_gamemode(&self, gamemode: &str) -> bool {
        self.server.set_gamemode(&self.name, gamemode)
    }

    /// Play a sound at this player's position (audible to nearby players too).
    pub fn play_sound(&self, sound_id: &str, volume: f32, pitch: f32) -> bool {
        self.server.play_sound_to_player(&self.name, sound_id, volume, pitch)
    }

    pub fn add_effect(
        &self,
        effect_id: &str,
        duration_ticks: i32,
        amplifier: u8,
        show_particles: bool,
    ) -> bool {
        self.entity().map_or(false, |e: Entity<'_>| {
            e.add_effect(effect_id, duration_ticks, amplifier, show_particles)
        })
    }

    pub fn remove_effect(&self, effect_id: &str) -> bool {
        self.entity().map_or(false, |e: Entity<'_>| e.remove_effect(effect_id))
    }

    pub fn clear_effects(&self) -> bool {
        self.entity().map_or(false, |e: Entity<'_>| e.clear_effects())
    }
}
