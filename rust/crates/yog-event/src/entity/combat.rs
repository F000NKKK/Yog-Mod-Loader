//! Entities hurt and hit.

use yog_data::Schema;

/// Fired after a living entity takes damage (server side).
#[derive(Debug, Clone, Schema)]
pub struct Damage {
    /// Registry id of the entity, e.g. `minecraft:zombie`.
    pub entity_type: String,
    pub uuid: String,
    /// Amount of damage dealt (hit points).
    pub amount: f32,
    /// Identifier of the damage source, e.g. `minecraft:player`, `fall`.
    pub source: String,
    pub dimension: String,
}

/// Fired when a persistent projectile (arrow, trident, etc.) hits a target.
///
/// - `Pre`  — return `false` to cancel the hit (projectile passes through).
/// - `Post` — hit was processed.
#[derive(Debug, Clone, Schema)]
pub struct ProjectileHit {
    /// Registry id of the projectile, e.g. `"minecraft:arrow"`.
    pub projectile_type: String,
    pub projectile_uuid: String,
    /// UUID of the entity that fired the projectile, or empty string.
    pub shooter_uuid: String,
    /// `"block"` or `"entity"`.
    pub hit_type: String,
    /// UUID of the entity that was hit (empty for block hits).
    pub hit_entity_uuid: String,
    pub x: f64,
    pub y: f64,
    pub z: f64,
    pub dimension: String,
}
