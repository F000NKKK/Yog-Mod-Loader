//! Particles: the host's actions.

use crate::Point;
use yog_ids as ids;

actions! {
    /// Spawns `count` particles around `pos`.
    SpawnParticles { dim: String, pos: Point, particle: String, count: i32, spread: Point, speed: f64 } -> bool = ids::service_particle::SPAWN, "spawnParticles";
}
