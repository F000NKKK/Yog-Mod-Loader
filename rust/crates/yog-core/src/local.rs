//! What this client's player is and sees (`yog_event::client::local`), and
//! how it clicks a container (`client::control`): records the host builds.

use yog_data::{Atom, Schema};

use crate::{BlockPos, Point};

/// This client's player: where it is and looks, how it moves and fares.
#[derive(Debug, Clone, PartialEq, Schema)]
#[data(view)]
pub struct LocalState {
    pub pos: Point,
    /// Degrees: yaw 0 looks south (+z), 90 west; pitch 90 looks down.
    pub yaw: f32,
    pub pitch: f32,
    /// Blocks a tick.
    pub velocity: Point,
    pub on_ground: bool,
    pub in_water: bool,
    pub eye_height: f64,
    pub health: f32,
    pub food: i32,
    pub dimension: Atom,
    /// The selected hotbar slot, 0–8.
    pub slot: u32,
    /// How charged the next attack is: 1 does full damage.
    pub attack_strength: f32,
}

impl LocalState {
    /// Where its eyes are.
    pub fn eye_pos(&self) -> Point {
        Point { y: self.pos.y + self.eye_height, ..self.pos }
    }
}

/// One kind of block in a [`BlockArea`]: how it is to walk through.
#[derive(Debug, Clone, PartialEq, Schema)]
#[data(view)]
pub struct BlockKind {
    pub id: Atom,
    /// Can be stood on and can't be walked through.
    pub solid: bool,
    pub liquid: bool,
    /// Ladders, vines: climbed.
    pub climbable: bool,
    /// Hurts to touch (fire, lava, cactus, berry bush…).
    pub harmful: bool,
}

/// The blocks between `min` and `max` (what isn't loaded reads as
/// bedrock): a palette and an index into it per block, x fastest, then z,
/// then y.
#[derive(Debug, Clone, PartialEq, Schema)]
#[data(view)]
pub struct BlockArea {
    pub min: BlockPos,
    pub max: BlockPos,
    pub palette: Vec<BlockKind>,
    pub blocks: Vec<u16>,
}

impl BlockArea {
    /// The kind of block at `pos`, if it's inside.
    pub fn at(&self, pos: BlockPos) -> Option<&BlockKind> {
        let (w, d) = (self.max.x - self.min.x + 1, self.max.z - self.min.z + 1);
        let (x, y, z) = (pos.x - self.min.x, pos.y - self.min.y, pos.z - self.min.z);
        let inside = (0..w).contains(&x) && (0..d).contains(&z) && (0..=self.max.y - self.min.y).contains(&y);
        let index = inside.then(|| self.blocks.get(((y * d + z) * w + x) as usize)).flatten()?;
        self.palette.get(usize::from(*index))
    }
}

/// An entity near this client's player.
#[derive(Debug, Clone, PartialEq, Schema)]
#[data(view)]
pub struct NearbyEntity {
    pub uuid: String,
    pub entity_type: Atom,
    pub pos: Point,
    pub eye_height: f64,
    /// Blocks a tick.
    pub velocity: Point,
    pub health: Option<f32>,
    /// A player's name.
    pub name: Option<String>,
    /// A monster, hostile to players.
    pub hostile: bool,
}

/// How a container slot is clicked: the game's own modes (`ClickType`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Schema)]
pub enum ClickMode {
    Pickup,
    QuickMove,
    Swap,
    Clone,
    Throw,
    QuickCraft,
    PickupAll,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_area_finds_its_blocks() {
        let kind = |id: &str| BlockKind { id: id.into(), solid: true, liquid: false, climbable: false, harmful: false };
        let area = BlockArea { min: BlockPos::new(0, 0, 0), max: BlockPos::new(1, 1, 0), palette: vec![kind("a"), kind("b")], blocks: vec![0, 1, 1, 0] };
        assert_eq!(area.at(BlockPos::new(1, 0, 0)).map(|k| k.id.as_str()), Some("b"));
        assert_eq!(area.at(BlockPos::new(0, 1, 0)).map(|k| k.id.as_str()), Some("b"));
        assert_eq!(area.at(BlockPos::new(2, 0, 0)), None);
    }
}
