//! The local player's movement: a mod may replace a travel step (what the
//! player moves by this tick) — gliding, flying, dashing. Runs on the
//! client, which moves its own player; the server takes the result.

use crate::math::Vec3;

/// The local player before a travel step.
#[derive(yog_data::Schema, Debug, Clone, PartialEq, Default)]
pub struct TravelFrame {
    /// The local player.
    pub uuid: String,
    /// Blocks per tick, as it is now.
    pub velocity: Vec3,
    /// Unit view direction.
    pub look: Vec3,
    /// Degrees.
    pub yaw: f32,
    pub pitch: f32,
    /// Movement keys: forward (−1–1), strafe left (−1–1), jump, sneak.
    pub forward: f32,
    pub strafe: f32,
    pub jumping: bool,
    pub sneaking: bool,
    /// Pull per tick (0.08, 0.01 with slow falling).
    pub gravity: f32,
    /// `entity_flags` bits.
    pub flags: u32,
}

/// Vanilla's elytra numbers (`LivingEntity.travel` while fall-flying).
mod glide {
    /// A look this long (vertical included) lifts fully.
    pub const FULL_LIFT_LOOK: f64 = 0.4;
    /// How much of gravity a level glide cancels.
    pub const LIFT: f64 = 0.75;
    /// Falling turned into forward speed.
    pub const FALL_TO_SPEED: f64 = 0.1;
    /// Speed turned into climb when looking up.
    pub const CLIMB: f64 = 0.04;
    /// Climb's vertical gain.
    pub const CLIMB_UP: f64 = 3.2;
    /// How fast the velocity turns toward the look.
    pub const STEER: f64 = 0.1;
    /// Drag a tick: across and down.
    pub const DRAG: f64 = 0.99;
    pub const DRAG_Y: f64 = 0.98;
}

/// Vanilla's elytra step (`LivingEntity.travel` while fall-flying): the
/// velocity to move by this tick, drag applied.
pub fn elytra(f: &TravelFrame) -> Vec3 {
    let v = f.velocity;
    let look = f.look;
    let pitch = f.pitch.to_radians();
    let g = f.gravity as f64;
    let (lx, ly, lz) = (look.x as f64, look.y as f64, look.z as f64);
    let (mut vx, mut vy, mut vz) = (v.x as f64, v.y as f64, v.z as f64);
    let d1 = (lx * lx + lz * lz).sqrt();
    let d3 = (vx * vx + vz * vz).sqrt();
    let d4 = (lx * lx + ly * ly + lz * lz).sqrt();
    let mut d5 = (pitch as f64).cos();
    d5 = d5 * d5 * (d4 / glide::FULL_LIFT_LOOK).min(1.0);
    vy += g * (-1.0 + d5 * glide::LIFT);
    if vy < 0.0 && d1 > 0.0 {
        let d6 = vy * -glide::FALL_TO_SPEED * d5;
        vx += lx * d6 / d1;
        vy += d6;
        vz += lz * d6 / d1;
    }
    if pitch < 0.0 && d1 > 0.0 {
        let d10 = d3 * (-(pitch.sin()) as f64) * glide::CLIMB;
        vx += -lx * d10 / d1;
        vy += d10 * glide::CLIMB_UP;
        vz += -lz * d10 / d1;
    }
    if d1 > 0.0 {
        vx += (lx / d1 * d3 - vx) * glide::STEER;
        vz += (lz / d1 * d3 - vz) * glide::STEER;
    }
    Vec3::new(
        (vx * glide::DRAG) as f32,
        (vy * glide::DRAG_Y) as f32,
        (vz * glide::DRAG) as f32,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gliding_level_falls_slowly_and_keeps_going() {
        let f = TravelFrame {
            velocity: Vec3::new(0.0, 0.0, 1.0),
            look: Vec3::new(0.0, 0.0, 1.0),
            gravity: 0.08,
            ..Default::default()
        };
        let v = elytra(&f);
        assert!(v.y < 0.0 && v.y > -0.08, "{v:?}");
        assert!(v.z > 0.9);
    }
}
