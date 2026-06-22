//! Content registration — declare custom items and blocks from Rust.
//!
//! Definitions are collected at registration time and handed to the host, which
//! registers real `Item`/`Block` objects before the game's registries freeze,
//! puts them in a "Yog" creative tab, and applies their properties.
//! (Textures, models and recipes are assets/data shipped in the `.yog` and
//! served to the game.)
//!
//! ```
//! # use yog_registry::{ItemDef, BlockDef, FoodDef};
//! ItemDef::new("mymod:ruby").name("Ruby").tooltip("A shiny gem.").max_stack(16);
//! ItemDef::new("mymod:pie").food(FoodDef::new(4, 0.3));
//! BlockDef::new("mymod:lamp").light_level(15).sound("stone");
//! ```

// ── Items ────────────────────────────────────────────────────────────────────

/// Nutritional properties for a food item.
#[derive(Debug, Clone)]
pub struct FoodDef {
    /// Hunger points restored (1 unit = half a drumstick).
    pub nutrition: u32,
    /// Saturation modifier applied after eating.
    pub saturation: f32,
    /// If `true`, edible even when the hunger bar is full.
    pub can_always_eat: bool,
}

impl FoodDef {
    pub fn new(nutrition: u32, saturation: f32) -> Self {
        Self { nutrition, saturation, can_always_eat: false }
    }

    pub fn can_always_eat(mut self) -> Self {
        self.can_always_eat = true;
        self
    }
}

/// A custom item to register, identified by `namespace:path`.
#[derive(Debug, Clone)]
pub struct ItemDef {
    pub id: String,
    pub max_stack: u8,
    pub name: Option<String>,
    pub tooltip: Option<String>,
    /// Durability. `0` = non-damageable. If set, `max_stack` is forced to 1.
    pub max_damage: u32,
    /// Immune to fire and lava damage (like netherite items).
    pub fire_resistant: bool,
    /// Furnace fuel burn time in ticks (`0` = not fuel; 200 = one coal equivalent).
    pub fuel_ticks: u32,
    /// Nutritional properties; `None` = not food.
    pub food: Option<FoodDef>,
}

impl ItemDef {
    pub fn new(id: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            max_stack: 64,
            name: None,
            tooltip: None,
            max_damage: 0,
            fire_resistant: false,
            fuel_ticks: 0,
            food: None,
        }
    }

    /// Maximum stack size (default 64). Ignored when `max_damage` is set.
    pub fn max_stack(mut self, n: u8) -> Self {
        self.max_stack = n;
        self
    }

    /// Display name shown in-game.
    pub fn name(mut self, name: impl Into<String>) -> Self {
        self.name = Some(name.into());
        self
    }

    /// A tooltip line shown on hover.
    pub fn tooltip(mut self, tooltip: impl Into<String>) -> Self {
        self.tooltip = Some(tooltip.into());
        self
    }

    /// Make this a damageable item (tool/weapon/armour). Forces stack size to 1.
    pub fn max_damage(mut self, durability: u32) -> Self {
        self.max_damage = durability;
        self
    }

    /// Make this item fire-resistant (won't burn in fire or lava).
    pub fn fire_resistant(mut self) -> Self {
        self.fire_resistant = true;
        self
    }

    /// Register as furnace fuel burning for `ticks` (200 ticks = 1 item smelted).
    pub fn fuel(mut self, ticks: u32) -> Self {
        self.fuel_ticks = ticks;
        self
    }

    /// Make this item edible with the given nutritional properties.
    pub fn food(mut self, food: FoodDef) -> Self {
        self.food = Some(food);
        self
    }
}

// ── Blocks ───────────────────────────────────────────────────────────────────

/// A custom block to register; it also gets a matching block-item.
#[derive(Debug, Clone)]
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
    /// Sound group id: `"stone"`, `"wood"`, `"grass"`, `"sand"`, `"snow"`,
    /// `"gravel"`, `"metal"`, `"glass"`, `"wool"`, `"nether_brick"`.
    /// `None` = stone (Minecraft default).
    pub sound: Option<String>,
    /// If `true`, the correct tool (from the block's tags) is required for drops.
    pub requires_tool: bool,
    /// If `true`, entities pass through this block (like flowers or torches).
    pub no_collision: bool,
    /// Friction coefficient. `0.0` = default (0.6). Ice = 0.989.
    pub slipperiness: f32,
}

impl BlockDef {
    pub fn new(id: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            hardness: 1.5,
            resistance: 6.0,
            name: None,
            shape: None,
            light_level: 0,
            sound: None,
            requires_tool: false,
            no_collision: false,
            slipperiness: 0.0,
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

    /// Custom hitbox/outline in pixel units (0–16).
    pub fn shape(mut self, x1: f32, y1: f32, z1: f32, x2: f32, y2: f32, z2: f32) -> Self {
        self.shape = Some([x1, y1, z1, x2, y2, z2]);
        self
    }

    /// Emitted light level (0–15).
    pub fn light_level(mut self, level: u8) -> Self {
        self.light_level = level.min(15);
        self
    }

    /// Sound group: `"stone"`, `"wood"`, `"grass"`, `"sand"`, `"snow"`,
    /// `"gravel"`, `"metal"`, `"glass"`, `"wool"`, `"nether_brick"`.
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
}
