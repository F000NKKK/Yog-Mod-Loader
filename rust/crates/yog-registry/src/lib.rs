//! Content registration — declare custom items and blocks from Rust.
//!
//! Definitions are collected at registration time and handed to the host, which
//! registers real `Item`/`Block` objects before the game's registries freeze,
//! puts them in per-namespace creative tabs, and applies their properties.
//! (Textures, models and recipes are assets/data shipped in the `.yog` and
//! served to the game.)
//!
//! ```
//! # use yog_registry::{ItemDef, BlockDef, FoodDef};
//! ItemDef::new("mymod:ruby").name("Ruby").tooltip("A shiny gem.").max_stack(16);
//! ItemDef::new("mymod:pie").food(FoodDef::new(4, 0.3));
//! BlockDef::new("mymod:lamp").light_level(15).sound("stone");
//! ```

#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};

// ── Items ────────────────────────────────────────────────────────────────────

/// Nutritional properties for a food item.
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
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
        Self {
            nutrition,
            saturation,
            can_always_eat: false,
        }
    }

    pub fn can_always_eat(mut self) -> Self {
        self.can_always_eat = true;
        self
    }
}

/// A custom item to register, identified by `namespace:path`.
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
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

// ── Recipes ──────────────────────────────────────────────────────────────────

/// A shaped crafting recipe (3×3 grid with a pattern and key mapping).
///
/// ```
/// # use yog_registry::ShapedRecipe;
/// ShapedRecipe::new("yog:ruby_sword", "yog:ruby_shard", 1)
///     .row("R  ").row("RS ").row(" S ")
///     .key('R', "yog:ruby_shard").key('S', "minecraft:stick");
/// ```
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[derive(Debug, Clone)]
pub struct ShapedRecipe {
    pub id: String,
    pub output: String,
    pub count: u32,
    rows: Vec<String>,
    keys: Vec<(char, String)>,
}

impl ShapedRecipe {
    pub fn new(id: impl Into<String>, output: impl Into<String>, count: u32) -> Self {
        Self {
            id: id.into(),
            output: output.into(),
            count,
            rows: Vec::new(),
            keys: Vec::new(),
        }
    }

    pub fn row(mut self, pattern: impl Into<String>) -> Self {
        self.rows.push(pattern.into());
        self
    }

    pub fn key(mut self, symbol: char, item_id: impl Into<String>) -> Self {
        self.keys.push((symbol, item_id.into()));
        self
    }

    /// Generate the Minecraft 1.20 recipe JSON for this recipe.
    pub fn to_json(&self) -> String {
        let pattern: String = self
            .rows
            .iter()
            .map(|r| format!("\"{}\"", r))
            .collect::<Vec<_>>()
            .join(",");
        let keys: String = self
            .keys
            .iter()
            .map(|(ch, item)| format!("\"{}\":{{\"item\":\"{}\"}}", ch, item))
            .collect::<Vec<_>>()
            .join(",");
        format!(
            "{{\"type\":\"minecraft:crafting_shaped\",\"pattern\":[{}],\"key\":{{{}}},\"result\":{{\"item\":\"{}\",\"count\":{}}}}}",
            pattern, keys, self.output, self.count
        )
    }

    /// Split `namespace:name` from the recipe id.
    pub fn ns_name(&self) -> (&str, &str) {
        self.id.split_once(':').unwrap_or(("minecraft", &self.id))
    }
}

/// A shapeless crafting recipe (unordered ingredients).
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[derive(Debug, Clone)]
pub struct ShapelessRecipe {
    pub id: String,
    pub output: String,
    pub count: u32,
    pub ingredients: Vec<String>,
}

impl ShapelessRecipe {
    pub fn new(id: impl Into<String>, output: impl Into<String>, count: u32) -> Self {
        Self {
            id: id.into(),
            output: output.into(),
            count,
            ingredients: Vec::new(),
        }
    }

    pub fn ingredient(mut self, item_id: impl Into<String>) -> Self {
        self.ingredients.push(item_id.into());
        self
    }

    pub fn to_json(&self) -> String {
        let ingr: String = self
            .ingredients
            .iter()
            .map(|i| format!("{{\"item\":\"{}\"}}", i))
            .collect::<Vec<_>>()
            .join(",");
        format!(
            "{{\"type\":\"minecraft:crafting_shapeless\",\"ingredients\":[{}],\"result\":{{\"item\":\"{}\",\"count\":{}}}}}",
            ingr, self.output, self.count
        )
    }

    pub fn ns_name(&self) -> (&str, &str) {
        self.id.split_once(':').unwrap_or(("minecraft", &self.id))
    }
}

/// A furnace smelting recipe.
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[derive(Debug, Clone)]
pub struct FurnaceRecipe {
    pub id: String,
    pub input: String,
    pub output: String,
    pub count: u32,
    pub experience: f32,
    pub cook_time: u32,
}

impl FurnaceRecipe {
    pub fn new(
        id: impl Into<String>,
        input: impl Into<String>,
        output: impl Into<String>,
        count: u32,
    ) -> Self {
        Self {
            id: id.into(),
            input: input.into(),
            output: output.into(),
            count,
            experience: 0.1,
            cook_time: 200,
        }
    }

    pub fn experience(mut self, xp: f32) -> Self {
        self.experience = xp;
        self
    }

    /// Cooking time in ticks (default 200 = 10 seconds).
    pub fn cook_time(mut self, ticks: u32) -> Self {
        self.cook_time = ticks;
        self
    }

    pub fn to_json(&self) -> String {
        format!(
            "{{\"type\":\"minecraft:smelting\",\"ingredient\":{{\"item\":\"{}\"}},\"result\":\"{}\",\"experience\":{},\"cookingtime\":{}}}",
            self.input, self.output, self.experience, self.cook_time
        )
    }

    pub fn ns_name(&self) -> (&str, &str) {
        self.id.split_once(':').unwrap_or(("minecraft", &self.id))
    }
}

// ── BookRecipe (like Patchouli's shapeless_book_recipe) ──────────────────────

/// A shapeless recipe that produces a book from `yog-book`.
/// Replaces `patchouli:shapeless_book_recipe`.
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[derive(Debug, Clone)]
pub struct BookRecipe {
    pub id: String,
    pub book: String,
    pub ingredients: Vec<String>,
}

impl BookRecipe {
    pub fn new(id: impl Into<String>, book: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            book: book.into(),
            ingredients: Vec::new(),
        }
    }

    pub fn ingredient(mut self, item_id: impl Into<String>) -> Self {
        self.ingredients.push(item_id.into());
        self
    }

    pub fn to_json(&self) -> String {
        let ingr: String = self
            .ingredients
            .iter()
            .map(|i| format!("{{\"item\":\"{}\"}}", i))
            .collect::<Vec<_>>()
            .join(",");
        format!(
            "{{\"type\":\"yog:crafting_book\",\"ingredients\":[{}],\"book\":\"{}\"}}",
            ingr, self.book
        )
    }

    pub fn ns_name(&self) -> (&str, &str) {
        self.id.split_once(':').unwrap_or(("yog", &self.id))
    }
}

// ── ItemModifier ─────────────────────────────────────────────────────────────

/// An item modifier applied during loot generation (like smelting, enchanted, etc.).
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[derive(Debug, Clone)]
pub struct ItemModifier {
    pub id: String,
    /// Modifier function identifier, e.g. "yog:set_count" or "hexcasting:amethyst_shard_reducer".
    pub function: String,
    /// Parameters as JSON object.
    pub parameters: std::collections::HashMap<String, String>,
}

impl ItemModifier {
    pub fn new(id: impl Into<String>, function: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            function: function.into(),
            parameters: std::collections::HashMap::new(),
        }
    }

    pub fn param(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.parameters.insert(key.into(), value.into());
        self
    }

    pub fn to_json(&self) -> String {
        let params: String = self
            .parameters
            .iter()
            .map(|(k, v)| format!("\"{}\":{}", k, v))
            .collect::<Vec<_>>()
            .join(",");
        format!(
            "{{\"function\":\"{}\",{}}}",
            self.function,
            if params.is_empty() {
                String::new()
            } else {
                params
            }
        )
    }
}

// ── StartupGrant ─────────────────────────────────────────────────────────────

/// Grant items/books to every player once when they first join.
/// This is the Yog-side replacement for `grant_patchi_book.json`.
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[derive(Debug, Clone)]
pub struct StartupGrant {
    pub id: String,
    pub items: Vec<String>,
    pub book: Option<String>,
    pub command: Option<String>,
}

impl StartupGrant {
    pub fn new(id: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            items: Vec::new(),
            book: None,
            command: None,
        }
    }

    pub fn item(mut self, item_id: impl Into<String>) -> Self {
        self.items.push(item_id.into());
        self
    }

    pub fn book(mut self, book: impl Into<String>) -> Self {
        self.book = Some(book.into());
        self
    }

    pub fn command(mut self, cmd: impl Into<String>) -> Self {
        self.command = Some(cmd.into());
        self
    }

    pub fn to_json(&self) -> String {
        let items: Vec<String> = self
            .items
            .iter()
            .map(|i| format!("{{\"item\":\"{}\"}}", i))
            .collect();
        let mut entries = String::new();
        if !items.is_empty() {
            entries.push_str(&format!("[{}]", items.join(",")));
        }
        if let Some(book) = &self.book {
            if !entries.is_empty() {
                entries.push(',');
            }
            let book_entry = format!(
                "{{\"type\":\"item\",\"name\":\"minecraft:written_book\",\"functions\":[{{\"function\":\"set_nbt\",\"tag\":\"{{yog_book:\\\"{}\\\"}}\"}}]}}",
                book
            );
            entries.push_str(&book_entry);
        }
        if let Some(cmd) = &self.command {
            if !entries.is_empty() {
                entries.push(',');
            }
            entries.push_str(&format!(
                "{{\"type\":\"minecraft:command\",\"command\":\"{}\"}}",
                cmd.replace('"', "\\\"")
            ));
        }
        format!(
            "{{\"type\":\"yog:startup_grant\",\"entries\":{},\"id\":\"{}\"}}",
            if entries.is_empty() {
                "[]".to_string()
            } else {
                entries
            },
            self.id
        )
    }
}

// ── Blocks ───────────────────────────────────────────────────────────────────
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[derive(Debug, Clone)]
pub struct AdvancementReward {
    pub id: String,
    /// The item to grant (default: special book item when book rewards).
    pub item: String,
    /// Optional NBT tag to apply.
    pub nbt: Option<String>,
    /// If set, the reward links to a yog-book.
    pub book: Option<String>,
}

impl AdvancementReward {
    pub fn new(id: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            item: "minecraft:written_book".into(),
            nbt: None,
            book: None,
        }
    }

    pub fn item(mut self, item: impl Into<String>) -> Self {
        self.item = item.into();
        self
    }

    pub fn nbt(mut self, nbt: impl Into<String>) -> Self {
        self.nbt = Some(nbt.into());
        self
    }

    pub fn book(mut self, book: impl Into<String>) -> Self {
        let book_str: String = book.into();
        self.book = Some(book_str.clone());
        self.nbt = Some(format!(
            "{{yog_book:\"{}\",title:\"Yog Book\",author:\"Yog\"}}",
            book_str
        ));
        self
    }

    pub fn to_json(&self) -> String {
        let nbt_part = self
            .nbt
            .as_ref()
            .map(|n| format!("{{\"function\":\"set_nbt\",\"tag\":{}}}", n));
        let entries = if let Some(nbt_part) = nbt_part {
            format!(
                "[{{\"type\":\"item\",\"name\":\"{}\",\"functions\":[{}]}}]",
                self.item, nbt_part
            )
        } else {
            format!("[{{\"type\":\"item\",\"name\":\"{}\"}}]", self.item)
        };
        format!(
            "{{\"type\":\"advancement_reward\",\"pools\":[{{\"rolls\":1,\"entries\":{}}}]}}",
            entries
        )
    }
}

// ── Blocks ───────────────────────────────────────────────────────────────────

/// A custom block to register; it also gets a matching block-item.
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
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
            connects: false,
            connect_groups: Vec::new(),
            inventory_id: None,
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
    /// generally) when their tag sets share at least one entry. Example: for
    /// Yog-VLSI, `analog_cable` and `redstone_port` both carry `"analog"`;
    /// `digital_cable` carries `"digital"`; an ALU block carries both, so it
    /// accepts either cable while a Redstone Port only accepts the analog one.
    pub fn connect_groups(mut self, groups: &[&str]) -> Self {
        self.connect_groups = groups.iter().map(|s| s.to_string()).collect();
        self
    }

    /// Back this block with a real Container/Menu inventory screen, whose
    /// shape/layout was registered separately via `Registry::register_inventory`
    /// — see `yog_inventory::InventoryDef`. `id` is that def's id.
    pub fn inventory(mut self, id: impl Into<String>) -> Self {
        self.inventory_id = Some(id.into());
        self
    }
}
