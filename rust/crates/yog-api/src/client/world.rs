//! The world as this client sees it: its blocks — found by kind, or an
//! area of them for finding a way — and the entities near the player.

use serde::Deserialize;
use serde_json::json;
use yog_ids::client_op as op;

use super::op::ask;

/// A block found by [`scan`]: where, and what.
#[derive(Deserialize, Debug, Clone, PartialEq)]
pub struct Found {
    pub x: i32,
    pub y: i32,
    pub z: i32,
    pub id: String,
}

/// The blocks among `ids` (`minecraft:diamond_ore`, `#minecraft:logs` for a tag) between `min` and `max`, in the loaded world.
pub fn scan(min: [i32; 3], max: [i32; 3], ids: &[&str]) -> Vec<Found> {
    ask(op::SCAN, json!({ "min": min, "max": max, "ids": ids })).unwrap_or_default()
}

/// One kind of block in an [`Area`].
#[derive(Deserialize, Debug, Clone, PartialEq)]
pub struct Kind {
    pub id: String,
    /// Can be stood on and can't be walked through.
    pub solid: bool,
    pub liquid: bool,
    /// Ladders, vines: climbed.
    pub climbable: bool,
    /// Hurts to touch (fire, lava, cactus, berry bush…).
    pub harmful: bool,
}

/// The blocks between `min` and `max`: a palette and an index into it per block, x fastest, then z, then y.
#[derive(Deserialize, Debug, Clone, PartialEq)]
pub struct Area {
    pub min: [i32; 3],
    pub max: [i32; 3],
    pub palette: Vec<Kind>,
    pub blocks: Vec<u16>,
}

impl Area {
    /// The kind of block at `pos`, if it's inside.
    pub fn at(&self, pos: [i32; 3]) -> Option<&Kind> {
        let [w, _, d] = [self.max[0] - self.min[0] + 1, 0, self.max[2] - self.min[2] + 1];
        let [x, y, z] = [pos[0] - self.min[0], pos[1] - self.min[1], pos[2] - self.min[2]];
        let inside = (0..w).contains(&x) && (0..d).contains(&z) && (0..=self.max[1] - self.min[1]).contains(&y);
        inside.then(|| self.blocks.get(((y * d + z) * w + x) as usize)).flatten().and_then(|i| self.palette.get(*i as usize))
    }
}

/// The blocks between `min` and `max` of the loaded world, for finding a way.
pub fn area(min: [i32; 3], max: [i32; 3]) -> Option<Area> {
    ask(op::AREA, json!({ "min": min, "max": max }))
}

/// An entity near the player.
#[derive(Deserialize, Debug, Clone, PartialEq)]
pub struct Entity {
    pub uuid: String,
    #[serde(rename = "type")]
    pub kind: String,
    pub x: f64,
    pub y: f64,
    pub z: f64,
    pub eye_height: f64,
    /// Its motion, blocks a tick (0 from a host that doesn't tell it).
    #[serde(default)]
    pub vx: f64,
    #[serde(default)]
    pub vy: f64,
    #[serde(default)]
    pub vz: f64,
    pub health: Option<f32>,
    /// A player's name.
    pub name: Option<String>,
    /// A monster, hostile to players.
    pub hostile: bool,
}

/// The entities within `radius` blocks of the player (not the player).
pub fn entities(radius: f64) -> Vec<Entity> {
    ask(op::ENTITIES, json!({ "radius": radius })).unwrap_or_default()
}

