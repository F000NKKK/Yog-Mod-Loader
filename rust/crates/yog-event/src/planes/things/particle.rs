//! Particles: the host's actions.

use crate::Point;
use yog_data::Schema;
use yog_ids as ids;

/// What a spawn makes: a particle type (the game's, `minecraft:smoke`, or a
/// mod's `.yparticle`) and what it's given — the host writes it as its
/// version takes it.
#[derive(Debug, Clone, PartialEq, Schema)]
pub struct ParticleOptions {
    pub id: String,
    /// `0xRRGGBB`: a mod's particle's colour, the game's dust's.
    pub color: Option<u32>,
    /// The game's dust's size (1 if not given).
    pub scale: Option<f32>,
}

impl ParticleOptions {
    /// Particle `id`, as it is.
    pub fn plain(id: &str) -> Self {
        ParticleOptions {
            id: id.to_owned(),
            color: None,
            scale: None,
        }
    }

    /// Particle `id` in `color` (`0xRRGGBB`).
    pub fn colored(id: &str, color: u32) -> Self {
        ParticleOptions {
            color: Some(color),
            ..Self::plain(id)
        }
    }
}

actions! {
    /// Spawns `count` particles around `pos` (none: one, `spread` its motion).
    SpawnParticles { dim: String, pos: Point, particle: ParticleOptions, count: i32, spread: Point, speed: f64 } -> bool = ids::service_particle::SPAWN, "actions.YogParticleActions.spawnParticles";
}
