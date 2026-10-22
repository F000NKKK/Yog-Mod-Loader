//! The game's own mirrors, typed: every entity and every player, as the
//! host writes them (`yog_mirror::host` is their source).
//!
//! ```ignore
//! let near = Entities::declare(Entities::POS | Entities::KIND, Rate::Every(1))?;
//! for e in near.iter() { draw(e.pos(), e.kind()); }
//! ```

use yog_core::Point;
use yog_data::Atom;

use crate::yog_mirror;

/// Every live entity, by network id.
#[yog_mirror(host = yog_mirror::host::ENTITY)]
pub struct Entities {
    pub pos: Point,
    /// Blocks a tick.
    pub velocity: Point,
    /// Yaw, pitch (degrees).
    pub rotation: [f32; 2],
    /// Its type.
    pub kind: Option<Atom>,
    pub dimension: Option<Atom>,
    /// Living ones'; others 0.
    pub health: f32,
    /// `[high, low]`.
    pub uuid: [u64; 2],
}

/// Every player (part of the entities: their facts read through it).
#[yog_mirror(host = yog_mirror::host::PLAYER)]
pub struct Players {
    pub name: Option<Atom>,
}

/// The blocks of the areas asked for, a dimension each: declared with
/// `Blocks::declare_in(dimension, Some(&BlockScope { min, max }), …)`.
#[yog_mirror(host = yog_mirror::host::BLOCK)]
pub struct Blocks {
    /// Its state: `minecraft:oak_stairs[facing=north,…]`.
    pub state: Option<Atom>,
}

/// The entities this client sees, by network id: written after its ticks,
/// on any server.
#[yog_mirror(host = yog_mirror::host::CLIENT_ENTITY)]
pub struct ClientEntities {
    pub pos: Point,
    /// Blocks a tick.
    pub velocity: Point,
    /// Yaw, pitch (degrees).
    pub rotation: [f32; 2],
    /// Its type.
    pub kind: Option<Atom>,
    /// Living ones'; others 0.
    pub health: f32,
    /// `[high, low]`.
    pub uuid: [u64; 2],
}

/// The blocks of the area asked for in this client's world, by packed
/// position (`ClientBlocks::declare_scoped(&BlockScope { min, max }, …)`):
/// each one's state and what a body meets there.
#[yog_mirror(host = yog_mirror::host::CLIENT_BLOCK)]
pub struct ClientBlocks {
    pub state: Option<Atom>,
    /// Can be stood on and can't be walked through.
    pub solid: u8,
    /// Water, lava…
    pub liquid: u8,
    /// Ladders, vines.
    pub climbable: u8,
    /// Hurts to touch (fire, lava, cactus…).
    pub harmful: u8,
}
