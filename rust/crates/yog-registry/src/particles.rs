//! A particle a mod adds (`.yparticle`): how it looks, how it moves and
//! how it's drawn — what the hosts draw it by, and Yog-IDLE's designer
//! previews. Its colour is each spawn's.

/// The game's own particles' defaults, where a particle doesn't say.
mod game {
    /// Speed kept each tick (`Particle.friction`).
    pub const FRICTION: f32 = 0.98;
    /// Ticks a particle lives, about (`Particle.lifetime`'s base).
    pub const LIFETIME: u32 = 20;
}

/// A particle type (`hexcasting:conjure_particle`): its id, and the rest.
#[derive(yog_data::Schema, Debug, Clone, PartialEq)]
pub struct ParticleDef {
    pub id: String,
    pub look: Particle,
}

/// A `.yparticle`: everything but its id (its path).
#[derive(yog_data::Schema, Debug, Clone, PartialEq)]
pub struct Particle {
    /// Its pictures: textures of the particles' atlas (`mymod:cloud`), in order.
    pub textures: Vec<String>,
    /// Which of them it shows.
    #[data(default)]
    pub frames: Frames,
    /// How much faster than its life its frames go by age (`Frames::Age`).
    #[data(default = "one")]
    pub frame_speed: f32,
    /// Once that faster run is past its life, its frames go by its age as it
    /// is (else they stay on the last).
    #[data(default)]
    pub frame_settle: bool,
    /// Ticks it lives: between `min` and `max`.
    #[data(default = "lifetime")]
    pub lifetime: Lifetime,
    /// Its size, times the game's particles' (which varies a little).
    #[data(default = "one")]
    pub size: f32,
    /// Its size times this each tick (1: it keeps it).
    #[data(default = "one")]
    pub size_per_tick: f32,
    /// Its opacity at first (0–1).
    #[data(default = "one", range = "0..=1")]
    pub alpha: f32,
    /// It fades out over its life, from `alpha` to none.
    #[data(default)]
    pub fade: bool,
    /// Its speed kept each tick.
    #[data(default = "friction")]
    pub friction: f32,
    /// Blocks a tick² it falls (negative: rises).
    #[data(default)]
    pub gravity: f32,
    /// It falls (`gravity`) only when spawned moving along every axis.
    #[data(default)]
    pub gravity_needs_motion: bool,
    /// It meets blocks (it doesn't go through them).
    #[data(default)]
    pub physics: bool,
    /// It speeds up sideways when something stops it rising or falling.
    #[data(default)]
    pub slide_when_blocked: bool,
    /// It starts turned at random (else upright).
    #[data(default)]
    pub random_roll: bool,
    /// How it's drawn.
    #[data(default)]
    pub render: ParticleRender,
}

/// Which picture a particle shows.
#[derive(yog_data::Schema, Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Frames {
    /// One at random, for its whole life.
    Random,
    /// One after another as it ages (`frame_speed` times as fast as it lives).
    #[default]
    Age,
}

/// The ticks a particle lives.
#[derive(yog_data::Schema, Debug, Clone, Copy, PartialEq, Eq)]
pub struct Lifetime {
    pub min: u32,
    pub max: u32,
}

/// How a particle is drawn.
#[derive(yog_data::Schema, Debug, Clone, Copy, PartialEq, Eq)]
pub struct ParticleRender {
    #[data(default)]
    pub blend: Blend,
    /// It hides what's behind it from later particles (off: they show through).
    #[data(default = "yes")]
    pub depth_write: bool,
    /// It glows: at full light whatever the light where it is.
    #[data(default)]
    pub glow: bool,
}

impl Default for ParticleRender {
    fn default() -> Self {
        ParticleRender {
            blend: Blend::default(),
            depth_write: true,
            glow: false,
        }
    }
}

/// How a particle's colour meets what's behind it.
#[derive(yog_data::Schema, Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Blend {
    /// It covers it (its picture's see-through pixels left out).
    Opaque,
    /// See-through as its alpha says.
    #[default]
    Translucent,
    /// Its light added to it: it glows over what's behind.
    Additive,
}

fn one() -> f32 {
    1.0
}

fn yes() -> bool {
    true
}

fn friction() -> f32 {
    game::FRICTION
}

fn lifetime() -> Lifetime {
    Lifetime {
        min: game::LIFETIME,
        max: game::LIFETIME,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_particle_says_only_what_isnt_the_games() {
        let p = yog_data::from_text::<Particle>(r#"{ textures: ["m:cloud"], render: { blend: "additive" } }"#)
            .unwrap()
            .value;
        assert_eq!(p.friction, game::FRICTION);
        assert_eq!(p.lifetime, lifetime());
        assert_eq!(p.frames, Frames::Age);
        assert_eq!(p.render.blend, Blend::Additive);
        assert!(p.render.depth_write && !p.render.glow);
    }
}
