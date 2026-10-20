//! What the world queries answer (`yog_event`'s entity, block, player and
//! world actions): records the host builds and the bus carries as Yog Data.

use yog_data::Schema;

use crate::{BlockPos, Point};

/// What kind of entity something is, as a bit set ([`EntityInfo::flags`]).
pub mod entity_flags {
    /// A `LivingEntity` (mobs, players, armor stands).
    pub const LIVING: u32 = 1;
    /// A player.
    pub const PLAYER: u32 = 1 << 1;
    /// An `Animal` or `WaterAnimal`.
    pub const ANIMAL: u32 = 1 << 2;
    /// An `Enemy` (hostile mobs).
    pub const MONSTER: u32 = 1 << 3;
    /// A dropped item (`ItemEntity`).
    pub const ITEM: u32 = 1 << 4;
    /// A `Mob` (has AI).
    pub const MOB: u32 = 1 << 5;
    /// An armor stand.
    pub const ARMOR_STAND: u32 = 1 << 6;
    /// Alive (not removed, not dead).
    pub const ALIVE: u32 = 1 << 7;
    /// A spectating player.
    pub const SPECTATOR: u32 = 1 << 8;
    /// A part of a multi-part entity (ender dragon parts).
    pub const PART: u32 = 1 << 9;
    /// A villager.
    pub const VILLAGER: u32 = 1 << 10;
    /// Sneaking (shift held).
    pub const SNEAKING: u32 = 1 << 11;
    /// Standing on the ground.
    pub const ON_GROUND: u32 = 1 << 12;
    /// Bumped into something sideways in its last move.
    pub const HORIZONTAL_COLLISION: u32 = 1 << 13;
    /// Fall-flying with an elytra.
    pub const GLIDING: u32 = 1 << 14;
}

/// A loaded entity (`entity::Info`).
#[derive(Debug, Clone, PartialEq, Schema)]
pub struct EntityInfo {
    /// Registry id of its type, e.g. `"minecraft:zombie"`.
    pub type_id: String,
    /// Dimension id, e.g. `"minecraft:overworld"`.
    pub dimension: String,
    /// Feet position.
    pub pos: Point,
    /// Eye height above `pos`.
    pub eye_height: f64,
    /// Bounding box width and height.
    pub width: f64,
    pub height: f64,
    /// [`entity_flags`] bits.
    pub flags: u32,
    /// Display name (custom name, player name, or the type's name).
    pub name: String,
}

impl EntityInfo {
    pub fn has(&self, flag: u32) -> bool {
        self.flags & flag != 0
    }

    /// Eye position.
    pub fn eye_pos(&self) -> Point {
        Point { y: self.pos.y + self.eye_height, ..self.pos }
    }
}

/// The six block faces, in Minecraft's `Direction` order (named as the
/// game names them: `down`, `up`, …).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Schema)]
pub enum Face {
    Down,
    Up,
    North,
    South,
    West,
    East,
}

impl Face {
    pub const ALL: [Face; 6] = [
        Face::Down,
        Face::Up,
        Face::North,
        Face::South,
        Face::West,
        Face::East,
    ];

    pub fn from_index(i: u8) -> Option<Self> {
        Self::ALL.get(i as usize).copied()
    }

    pub fn index(self) -> u8 {
        self as u8
    }

    /// Unit step out of the face.
    pub fn normal(self) -> (i32, i32, i32) {
        match self {
            Face::Down => (0, -1, 0),
            Face::Up => (0, 1, 0),
            Face::North => (0, 0, -1),
            Face::South => (0, 0, 1),
            Face::West => (-1, 0, 0),
            Face::East => (1, 0, 0),
        }
    }
}

/// A block a ray stopped at.
#[derive(Debug, Clone, Copy, PartialEq, Schema)]
pub struct BlockHit {
    pub pos: BlockPos,
    /// The face the ray entered through.
    pub face: Face,
    /// The exact hit point (on the block's surface).
    pub hit: Point,
}

/// A block's properties (`block::Info`).
#[derive(Debug, Clone, PartialEq, Schema)]
pub struct BlockInfo {
    /// Full block state, e.g. `"minecraft:oak_log[axis=y]"`.
    pub state: String,
    /// Destroy time; negative for unbreakable blocks (bedrock).
    pub hardness: f32,
    /// Placing a block here replaces this one (air, grass, water…).
    pub replaceable: bool,
    /// Air of any kind.
    pub air: bool,
    /// Id of the fluid in this block (`"minecraft:water"`), if any.
    pub fluid: Option<String>,
    /// Whether that fluid is a source block.
    pub fluid_source: bool,
    /// Redstone power the block receives (0–15).
    pub redstone: u8,
    /// Inside the world's build height and world border.
    pub in_world: bool,
}

impl BlockInfo {
    /// Registry id of the block, without its state properties.
    pub fn block_id(&self) -> &str {
        self.state.split('[').next().unwrap_or_default()
    }
}

/// An online player's state (`player::Info`).
#[derive(Debug, Clone, PartialEq, Schema)]
pub struct PlayerInfo {
    pub uuid: String,
    /// `"survival"`, `"creative"`, `"adventure"` or `"spectator"`.
    pub gamemode: String,
    /// Selected hotbar slot (0–8).
    pub selected_slot: u32,
    pub xp_level: i32,
    /// Total experience points.
    pub xp_total: i32,
    /// Air supply in ticks (300 when full).
    pub air: i32,
    /// Whether the player may fly (`abilities.mayfly`).
    pub may_fly: bool,
    pub flying: bool,
}

impl PlayerInfo {
    pub fn is_creative(&self) -> bool {
        self.gamemode == "creative"
    }
}

/// A stack of items: what it is, how many, its data (SNBT).
#[derive(Debug, Clone, PartialEq, Schema)]
pub struct Stack {
    pub item: String,
    pub count: u32,
    /// `{}` when it has none.
    pub snbt: String,
}

/// A stack in an inventory slot.
#[derive(Debug, Clone, PartialEq, Schema)]
pub struct SlotStack {
    pub slot: u32,
    pub stack: Stack,
}

/// A slot of an open inventory screen: what it holds, where it's drawn
/// (GUI pixels from the screen's corner).
#[derive(Debug, Clone, PartialEq, Schema)]
pub struct ScreenSlot {
    pub stack: Option<Stack>,
    pub x: i32,
    pub y: i32,
}

/// Which way an entity faces, in degrees.
#[derive(Debug, Clone, Copy, PartialEq, Default, Schema)]
pub struct Rotation {
    pub yaw: f32,
    pub pitch: f32,
}

/// A block found in the world: where, and its state.
#[derive(Debug, Clone, PartialEq, Schema)]
pub struct PlacedBlock {
    pub pos: BlockPos,
    /// Full block state, e.g. `"minecraft:oak_log[axis=y]"`.
    pub state: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn round_trip<T: Schema>(v: &T) -> T {
        yog_data::read_bytes(&yog_data::binary::encode(&v.write())).unwrap()
    }

    #[test]
    fn records_round_trip_as_data() {
        let e = EntityInfo {
            type_id: "minecraft:zombie".into(),
            dimension: "minecraft:overworld".into(),
            pos: Point { x: 1.5, y: 64.0, z: -2.0 },
            eye_height: 1.74,
            width: 0.6,
            height: 1.95,
            flags: entity_flags::LIVING | entity_flags::MONSTER,
            name: "Zom\tbie".into(),
        };
        assert!(e.has(entity_flags::LIVING) && !e.has(entity_flags::PLAYER));
        assert_eq!(e.eye_pos().y, 65.74);
        assert_eq!(round_trip(&e), e);
        let h = BlockHit { pos: BlockPos::new(1, 2, 3), face: Face::East, hit: Point { x: 1.0, y: 2.5, z: 3.5 } };
        assert_eq!(round_trip(&h), h);
        assert_eq!(h.face.normal(), (1, 0, 0));
    }

    #[test]
    fn faces_are_named_as_the_game_names_them() {
        let names: Vec<String> = Face::ALL.iter().map(|f| yog_data::text::to_string(&f.write())).collect();
        assert_eq!(names, ["\"down\"", "\"up\"", "\"north\"", "\"south\"", "\"west\"", "\"east\""]);
    }
}
