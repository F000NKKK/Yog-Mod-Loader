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
