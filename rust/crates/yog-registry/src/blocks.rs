//! Blocks: what a mod declares a block as.

#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};

/// A custom block to register; it also gets a matching block-item.
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[derive(yog_data::Schema, Debug, Clone)]
pub struct BlockDef {
    pub id: String,
    pub hardness: f32,
    pub resistance: f32,
    pub name: Option<String>,
    /// Optional collision/outline box in pixel units (0–16): `[x1,y1,z1,x2,y2,z2]`.
    /// `None` = full cube.
    pub shape: Option<[f32; 6]>,
    /// Light emitted by this block (0 = none, 15 = max, like a torch).
    pub light_level: u8,
    /// Its [`SoundGroup`]: one of the game's by name (`"stone"`,
    /// `"amethyst"`, `"copper"` — [`SoundGroup::built_in`]) or a mod's
    /// (`"mymod:crystal"`). `None` = stone (Minecraft default).
    pub sound: Option<String>,
    /// If `true`, the correct tool (from the block's tags) is required for drops.
    pub requires_tool: bool,
    /// If `true`, entities pass through this block (like flowers or torches).
    pub no_collision: bool,
    /// Friction coefficient. `0.0` = default (0.6). Ice = 0.989.
    pub slipperiness: f32,
    /// If `true`, this block dynamically grows arms toward neighbors it's
    /// compatible with (fence/pipe-style): the Java host tracks N/S/E/W/U/D
    /// boolean blockstate properties, recomputed on placement and on
    /// neighbor change, and grows the collision/outline shape from the
    /// `shape` core box (or a default post) out to each connected side.
    pub connects: bool,
    /// Connection compatibility tags (configured in code via
    /// `.connect_groups(&[...])` — see that method's docs for how this
    /// drives which blocks link up). Independent of `connects`: a block can
    /// carry tags purely as a connection *target* (e.g. an ALU accepting a
    /// Digital Cable) without dynamically growing its own shape.
    pub connect_groups: Vec<String>,
    /// Id of a `YogInventoryDef` this block is backed by — `None` = plain
    /// block (default). See `yog_inventory`'s DESIGN.md.
    pub inventory_id: Option<String>,
    /// Gives off redstone power — how much, per position, the mod sets with
    /// `block::set_redstone_output`.
    pub redstone_source: bool,
    /// How it turns when placed (a `facing` property): `""` (it doesn't),
    /// `"horizontal"` or `"all"`.
    pub facing: String,
    /// `"toward"`: its front toward the placer; `"away"`: the way they look.
    pub placement: String,
    /// It hangs on a floor, wall or ceiling, as a button (a `face`
    /// property and a horizontal `facing`).
    pub attach: bool,
    /// It can stand in water (a `waterlogged` property).
    pub waterlogged: bool,
    /// Properties of its own.
    pub properties: Vec<BlockProperty>,
    /// A yes/no property the block keeps on while it gets redstone power.
    pub powered: Option<String>,
    /// A yes/no property a comparator reads 15 from while it's on.
    pub comparator: Option<String>,
    /// Hoppers and droppers can put items into it (`on_item_insert`).
    pub accepts_items: bool,
    /// It doesn't hide the faces behind it or stop light (as glass, a door).
    #[data(default)]
    pub no_occlusion: bool,
    /// One of the game's kinds of block it is (`stairs`, `slab`, `door`,
    /// `trapdoor`, `button`, `pressure_plate`, `fence`, `fence_gate`): the
    /// host makes it of the game's class, with that class's states; `""`: a
    /// block of Yog's own.
    #[data(default)]
    pub kind: String,
    /// The block a kind is made from (stairs act as it).
    #[data(default)]
    pub base: Option<String>,
    /// A kind's set — a registered block set by name (`oak`, `iron`…; a
    /// fence gate's wood type); `None`: `oak`.
    #[data(default)]
    pub set: Option<String>,
    /// How long a button stays pressed, ticks.
    #[data(default = "thirty")]
    pub press_ticks: u32,
    /// Only living things set off a button or a pressure plate (not items
    /// or arrows).
    #[data(default)]
    pub mobs_only: bool,
}

fn thirty() -> u32 {
    30
}

/// A property of a block's own: its name and values (the first the
/// default; `false`, `true` is a yes/no).
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[derive(yog_data::Schema, Debug, Clone, PartialEq, Eq)]
pub struct BlockProperty {
    pub name: String,
    pub values: Vec<String>,
}

/// A block's hardness and blast resistance unless it says: stone's.
const STONE_HARDNESS: f32 = 1.5;
const STONE_RESISTANCE: f32 = 6.0;

impl BlockDef {
    pub fn new(id: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            hardness: STONE_HARDNESS,
            resistance: STONE_RESISTANCE,
            name: None,
            shape: None,
            light_level: 0,
            sound: None,
            requires_tool: false,
            no_collision: false,
            slipperiness: 0.0,
            connects: false,
            connect_groups: Vec::new(),
            inventory_id: None,
            redstone_source: false,
            facing: String::new(),
            placement: String::new(),
            attach: false,
            waterlogged: false,
            properties: Vec::new(),
            powered: None,
            comparator: None,
            accepts_items: false,
            no_occlusion: false,
            kind: String::new(),
            base: None,
            set: None,
            press_ticks: thirty(),
            mobs_only: false,
        }
    }

    /// Mining hardness and blast resistance (defaults 1.5 / 6.0).
    pub fn strength(mut self, hardness: f32, resistance: f32) -> Self {
        self.hardness = hardness;
        self.resistance = resistance;
        self
    }

    /// Display name shown in-game.
    pub fn name(mut self, name: impl Into<String>) -> Self {
        self.name = Some(name.into());
        self
    }

    /// Custom hitbox/outline in pixel units (0–16): its lowest corner and its highest.
    pub fn shape(mut self, [x1, y1, z1]: [f32; 3], [x2, y2, z2]: [f32; 3]) -> Self {
        self.shape = Some([x1, y1, z1, x2, y2, z2]);
        self
    }

    /// Emitted light level (0–15).
    pub fn light_level(mut self, level: u8) -> Self {
        self.light_level = level.min(15);
        self
    }

    /// Sound group: one of the game's (`"stone"`, `"amethyst"`…) or a
    /// mod's [`SoundGroup`] (`"mymod:crystal"`).
    pub fn sound(mut self, group: impl Into<String>) -> Self {
        self.sound = Some(group.into());
        self
    }

    /// Correct tool required for loot drops (equivalent to `requiresTool()`).
    pub fn requires_tool(mut self) -> Self {
        self.requires_tool = true;
        self
    }

    /// No physical collision — entities pass through (like flowers).
    pub fn no_collision(mut self) -> Self {
        self.no_collision = true;
        self
    }

    /// Friction (default 0.6). Set to 0.989 for ice-like slipperiness.
    pub fn slipperiness(mut self, value: f32) -> Self {
        self.slipperiness = value;
        self
    }

    /// Dynamically grow arms toward compatible neighbors — fence/pipe-style.
    /// "Compatible" means the neighbor also has a `connect_groups` tag in
    /// common (set here, or via `connect_groups` alone on a static target
    /// block). Call `.connect_groups(&[...])` too, or this connects to
    /// nothing. See the `connects` field doc.
    pub fn connects_to_neighbors(mut self) -> Self {
        self.connects = true;
        self
    }

    /// Connection compatibility tags — two blocks link up (for
    /// `connects_to_neighbors` arm growth, and as valid connection targets
    /// generally) when their tag sets share at least one entry. Example:
    /// `analog_cable` and `redstone_port` both carry `"analog"`;
    /// `digital_cable` carries `"digital"`; an ALU block carries both, so it
    /// accepts either cable while a Redstone Port only accepts the analog one.
    pub fn connect_groups(mut self, groups: &[&str]) -> Self {
        self.connect_groups = groups
            .iter()
            .map(std::string::ToString::to_string)
            .collect();
        self
    }

    /// Back this block with a real Container/Menu inventory screen, whose
    /// shape/layout was registered separately via `Registry::register_inventory`
    /// — see `yog_inventory::InventoryDef`. `id` is that def's id.
    /// A redstone power source: what each placed block gives off is set
    /// with `block::set_redstone_output` (0 until then).
    pub fn redstone_source(mut self) -> Self {
        self.redstone_source = true;
        self
    }

    /// Turns when placed: `"horizontal"` or `"all"` ways, its front (the
    /// model's north side) `"toward"` the placer or `"away"` (the way they look).
    pub fn facing(mut self, ways: &str, placement: &str) -> Self {
        self.facing = ways.to_string();
        self.placement = placement.to_string();
        self
    }

    /// Hangs on a floor, wall or ceiling, as a button.
    pub fn attach(mut self) -> Self {
        self.attach = true;
        self
    }

    /// Can stand in water.
    pub fn waterlogged(mut self) -> Self {
        self.waterlogged = true;
        self
    }

    /// A property of its own: `values`, the first the default.
    pub fn property(mut self, name: &str, values: &[&str]) -> Self {
        self.properties.push(BlockProperty {
            name: name.into(),
            values: values.iter().map(|v| v.to_string()).collect(),
        });
        self
    }

    /// Keeps yes/no property `name` on while it gets redstone power.
    pub fn powered(mut self, name: &str) -> Self {
        self.powered = Some(name.into());
        self
    }

    /// A comparator reads 15 from it while yes/no property `name` is on.
    pub fn comparator(mut self, name: &str) -> Self {
        self.comparator = Some(name.into());
        self
    }

    /// Hoppers and droppers can put items into it (`on_item_insert`).
    pub fn accepts_items(mut self) -> Self {
        self.accepts_items = true;
        self
    }

    pub fn inventory(mut self, id: impl Into<String>) -> Self {
        self.inventory_id = Some(id.into());
        self
    }

    /// Doesn't hide the faces behind it or stop light (as glass).
    pub fn no_occlusion(mut self) -> Self {
        self.no_occlusion = true;
        self
    }

    /// One of the game's kinds of block (`"stairs"`, `"door"`…).
    pub fn kind(mut self, kind: &str) -> Self {
        self.kind = kind.into();
        self
    }

    /// The block a kind is made from.
    pub fn base(mut self, id: impl Into<String>) -> Self {
        self.base = Some(id.into());
        self
    }

    /// A kind's block set (`"oak"`, `"dark_oak"`, `"iron"`…).
    pub fn set(mut self, name: &str) -> Self {
        self.set = Some(name.into());
        self
    }

    /// How long a button stays pressed, ticks (30 by default).
    pub fn press_ticks(mut self, ticks: u32) -> Self {
        self.press_ticks = ticks;
        self
    }

    /// Only living things set off a button or a pressure plate.
    pub fn mobs_only(mut self) -> Self {
        self.mobs_only = true;
        self
    }
}
