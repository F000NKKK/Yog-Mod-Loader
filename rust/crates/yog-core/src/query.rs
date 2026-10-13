//! Records returned by the world-query part of [`Server`](crate::Server)
//! (ABI minor 1), and their wire form.
//!
//! The Java host answers these queries with one tab-separated line; the
//! `decode` functions here are the single place that reads that line, so the
//! runtime and the tests agree on it.

use crate::BlockPos;

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
}

/// A loaded entity, as [`Server::entity_info`](crate::Server::entity_info)
/// sees it.
#[derive(Debug, Clone, PartialEq)]
pub struct EntityInfo {
    /// Registry id of its type, e.g. `"minecraft:zombie"`.
    pub type_id: String,
    /// Dimension id, e.g. `"minecraft:overworld"`.
    pub dimension: String,
    /// Feet position.
    pub pos: (f64, f64, f64),
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
    pub fn eye_pos(&self) -> (f64, f64, f64) {
        (self.pos.0, self.pos.1 + self.eye_height, self.pos.2)
    }

    /// `type\tdim\tx\ty\tz\teye\twidth\theight\tflags\tname`.
    pub fn decode(line: &str) -> Option<Self> {
        let mut it = line.splitn(10, '\t');
        let type_id = it.next()?.to_owned();
        let dimension = it.next()?.to_owned();
        let mut num = || it.next()?.parse::<f64>().ok();
        let pos = (num()?, num()?, num()?);
        let eye_height = num()?;
        let width = num()?;
        let height = num()?;
        let flags = it.next()?.parse().ok()?;
        let name = it.next().unwrap_or_default().to_owned();
        Some(Self {
            type_id,
            dimension,
            pos,
            eye_height,
            width,
            height,
            flags,
            name,
        })
    }
}

/// The six block faces, in Minecraft's `Direction` order.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
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
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BlockHit {
    pub pos: BlockPos,
    /// The face the ray entered through.
    pub face: Face,
    /// The exact hit point (on the block's surface).
    pub hit: (f64, f64, f64),
}

impl BlockHit {
    /// `x\ty\tz\tface\thx\thy\thz`.
    pub fn decode(line: &str) -> Option<Self> {
        let mut it = line.split('\t');
        let mut int = || it.next()?.parse::<i32>().ok();
        let pos = BlockPos::new(int()?, int()?, int()?);
        let face = Face::from_index(u8::try_from(int()?).ok()?)?;
        let mut num = || it.next()?.parse::<f64>().ok();
        let hit = (num()?, num()?, num()?);
        Some(Self { pos, face, hit })
    }
}

/// A block's properties, as [`Server::block_info`](crate::Server::block_info)
/// sees them.
#[derive(Debug, Clone, PartialEq)]
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

    /// `state\thardness\treplaceable\tair\tfluid\tsource\tredstone\tin_world`
    /// (booleans as `0`/`1`, an empty `fluid` for none).
    pub fn decode(line: &str) -> Option<Self> {
        let mut it = line.split('\t');
        let state = it.next()?.to_owned();
        let hardness = it.next()?.parse().ok()?;
        let flag = |s: Option<&str>| s.map(|s| s == "1");
        let replaceable = flag(it.next())?;
        let air = flag(it.next())?;
        let fluid = it.next()?;
        let fluid = (!fluid.is_empty()).then(|| fluid.to_owned());
        let fluid_source = flag(it.next())?;
        let redstone = it.next()?.parse().ok()?;
        let in_world = flag(it.next())?;
        Some(Self {
            state,
            hardness,
            replaceable,
            air,
            fluid,
            fluid_source,
            redstone,
            in_world,
        })
    }
}

/// An online player's state, as
/// [`Server::player_info`](crate::Server::player_info) sees it.
#[derive(Debug, Clone, PartialEq)]
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

    /// `uuid\tgamemode\tslot\tlevel\ttotal\tair\tmay_fly\tflying`.
    pub fn decode(line: &str) -> Option<Self> {
        let mut it = line.split('\t');
        let uuid = it.next()?.to_owned();
        let gamemode = it.next()?.to_owned();
        let selected_slot = it.next()?.parse().ok()?;
        let xp_level = it.next()?.parse().ok()?;
        let xp_total = it.next()?.parse().ok()?;
        let air = it.next()?.parse().ok()?;
        let may_fly = it.next()? == "1";
        let flying = it.next()? == "1";
        Some(Self {
            uuid,
            gamemode,
            selected_slot,
            xp_level,
            xp_total,
            air,
            may_fly,
            flying,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn records_decode() {
        let e = EntityInfo::decode(
            "minecraft:zombie\tminecraft:overworld\t1.5\t64\t-2\t1.74\t0.6\t1.95\t169\tZom\tbie",
        )
        .unwrap();
        assert_eq!(e.pos, (1.5, 64.0, -2.0));
        assert!(e.has(entity_flags::LIVING) && e.has(entity_flags::MONSTER));
        assert!(!e.has(entity_flags::PLAYER));
        assert_eq!(e.name, "Zom\tbie");
        assert_eq!(e.eye_pos().1, 65.74);

        let h = BlockHit::decode("1\t2\t3\t5\t1.0\t2.5\t3.5").unwrap();
        assert_eq!(h.face, Face::East);
        assert_eq!(h.face.normal(), (1, 0, 0));

        let b = BlockInfo::decode("minecraft:water[level=0]\t100\t1\t0\tminecraft:water\t1\t3\t1")
            .unwrap();
        assert_eq!(b.block_id(), "minecraft:water");
        assert_eq!(b.fluid.as_deref(), Some("minecraft:water"));
        assert!(b.fluid_source && b.replaceable && b.in_world && !b.air);
        assert_eq!(b.redstone, 3);

        let p = PlayerInfo::decode("u\tcreative\t4\t30\t1395\t300\t1\t0").unwrap();
        assert!(p.is_creative() && p.may_fly && !p.flying);
        assert_eq!(p.selected_slot, 4);

        assert!(EntityInfo::decode("x\ty\tnope").is_none());
        assert!(BlockHit::decode("1\t2\t3\t9\t0\t0\t0").is_none());
    }
}
