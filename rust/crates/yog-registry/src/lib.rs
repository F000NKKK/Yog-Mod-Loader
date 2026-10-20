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

pub mod effects;
pub mod recipe_kinds;
pub mod loot;
pub mod atlases;
pub mod item_looks;
pub mod tool_levels;

/// What a `.yog` registers when the game starts — content the game can't
/// add or change while it runs (blocks, items, entity types, sound groups,
/// materials, dimensions): a line each, `<kind> <id> <hash>`, the hash of
/// what can't change. A hot reload compares the new `.yog`'s with what's
/// registered ([`registration_changes`]).
pub const REGISTRATIONS_FILE: &str = "registrations.txt";

/// How `new` (a [`REGISTRATIONS_FILE`]) differs from `registered`: what it
/// adds, removes and changes — `<kind> <id>` each.
pub fn registration_changes(registered: &str, new: &str) -> (Vec<String>, Vec<String>, Vec<String>) {
    fn read(text: &str) -> std::collections::BTreeMap<String, String> {
        text.lines()
            .filter_map(|l| {
                let (key, hash) = l.trim().rsplit_once(' ')?;
                Some((key.to_string(), hash.to_string()))
            })
            .collect()
    }
    let (old, new) = (read(registered), read(new));
    let added = new.keys().filter(|k| !old.contains_key(*k)).cloned().collect();
    let removed = old.keys().filter(|k| !new.contains_key(*k)).cloned().collect();
    let changed = new.iter().filter(|(k, h)| old.get(*k).is_some_and(|o| o != *h)).map(|(k, _)| k.clone()).collect();
    (added, removed, changed)
}
pub use effects::{AttributeDef, Brew, EffectCategory, EffectDef, EffectModifier, ModifierOperation, PotionDef, PotionEffect};
pub use loot::{Loot, LootAdd, LootModifier, LootScale, LootStack};
pub use atlases::Atlas;
pub use item_looks::{ItemLooks, ItemTint, LookModels};
pub use tool_levels::{LevelTag, ToolLadder, ToolLevel};

// ── Items ────────────────────────────────────────────────────────────────────

/// Nutritional properties for a food item.
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[derive(yog_data::Schema, Debug, Clone)]
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
#[derive(yog_data::Schema, Debug, Clone)]
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
    /// A tool or a weapon of a material.
    pub tool: Option<ToolItem>,
    /// A piece of armor of a material.
    pub armor: Option<ArmorPiece>,
    /// Worn in this slot without being armor: `head`, `chest`, `legs`, `feet`.
    pub wearable: Option<String>,
    /// Used on a block's side, it hangs there as an entity of a hanging type.
    pub hangs: Option<Hangs>,
}

/// What an item hangs on a wall ([`ItemDef::hangs`]).
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[derive(yog_data::Schema, Debug, Clone, PartialEq)]
pub struct Hangs {
    /// A hanging entity type ([`EntityKind::Hanging`]).
    pub entity: String,
    /// Whole blocks of wall, wide and high; `None`: the type's size.
    pub size: Option<[u32; 2]>,
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
            tool: None,
            armor: None,
            wearable: None,
            hangs: None,
        }
    }

    /// A tool (`pickaxe`, `axe`, `shovel`, `hoe`, a mod's mineable tag
    /// `mymod:mineable/hammer`) or a `sword`, of `material` (a
    /// [`ToolMaterial`]: the game's `iron`… or a mod's).
    pub fn tool(mut self, kind: impl Into<String>, material: impl Into<String>, attack_damage: f32, attack_speed: f32) -> Self {
        self.tool = Some(ToolItem { kind: kind.into(), material: material.into(), attack_damage, attack_speed });
        self
    }

    /// A piece of armor for `slot` (`head`, `chest`, `legs`, `feet`) of
    /// `material` (an [`ArmorMaterial`]: the game's `diamond`… or a mod's).
    pub fn armor(mut self, slot: impl Into<String>, material: impl Into<String>) -> Self {
        self.armor = Some(ArmorPiece { slot: slot.into(), material: material.into() });
        self
    }

    /// Worn in `slot` (`head`, `chest`, `legs`, `feet`) without being armor.
    pub fn wearable(mut self, slot: impl Into<String>) -> Self {
        self.wearable = Some(slot.into());
        self
    }

    /// Used on a block's side, it hangs an `entity` (a hanging type) there,
    /// `size` blocks wide and high (`None`: the type's own); the entity
    /// keeps one of this item and drops it when it breaks.
    pub fn hangs(mut self, entity: impl Into<String>, size: Option<[u32; 2]>) -> Self {
        self.hangs = Some(Hangs { entity: entity.into(), size });
        self
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



/// How a cooking recipe cooks.
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Cooking {
    Smelting,
    Blasting,
    Smoking,
    Campfire,
}

impl Cooking {
    /// Every one, in [`recipe_kinds::ids::COOKING`]'s order.
    pub const ALL: [Cooking; 4] = [Cooking::Smelting, Cooking::Blasting, Cooking::Smoking, Cooking::Campfire];

    /// Its kind of recipe (`smelting`).
    pub fn word(self) -> &'static str {
        recipe_kinds::ids::COOKING[self as usize]
    }

    /// The cooking of kind `id`, if `id` is one.
    pub fn of_kind(id: &str) -> Option<Cooking> {
        Cooking::ALL.into_iter().find(|c| c.word() == id)
    }
}

/// What a recipe is made of besides its result.
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[derive(Debug, Clone, PartialEq)]
pub enum RecipeKind {
    /// A crafting grid: rows of key characters (space = empty).
    Shaped { pattern: Vec<String>, key: Vec<(char, Vec<String>)> },
    /// Crafting, in any arrangement.
    Shapeless { ingredients: Vec<Vec<String>> },
    Cooking { method: Cooking, ingredient: Vec<String>, experience: f32, cook_time: u32 },
    Stonecutting { ingredient: Vec<String> },
    /// A smithing table: `base` upgraded with `addition`, `template` used up.
    Smithing { template: Vec<String>, base: Vec<String>, addition: Vec<String> },
}

/// A recipe, in Yog's own format — what a `.yrecipe` says. The host writes
/// the recipe JSON of its Minecraft version from it.
///
/// An ingredient is an item id (`minecraft:stick`), an item tag (`#c:dusts`)
/// or — the `_any` builders — any of several.
///
/// ```
/// # use yog_registry::{Cooking, Recipe};
/// Recipe::shaped("yog:ruby_sword", "yog:ruby_sword", 1)
///     .row(" R ").row(" R ").row(" S ")
///     .key('R', "yog:ruby").key('S', "minecraft:stick");
/// Recipe::cooking("yog:ruby", Cooking::Blasting, "#c:ruby_ores", "yog:ruby", 1).experience(0.7);
/// ```
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[derive(Debug, Clone, PartialEq)]
pub struct Recipe {
    pub id: String,
    pub kind: RecipeKind,
    pub result: String,
    pub count: u32,
    /// Recipes of a group share a slot in the recipe book.
    pub group: Option<String>,
    /// The recipe book's tab (`building`, `equipment`, `misc`, …).
    pub category: Option<String>,
}

impl Recipe {
    fn of(id: impl Into<String>, kind: RecipeKind, result: impl Into<String>, count: u32) -> Self {
        Recipe { id: id.into(), kind, result: result.into(), count, group: None, category: None }
    }

    pub fn shaped(id: impl Into<String>, result: impl Into<String>, count: u32) -> Self {
        Self::of(id, RecipeKind::Shaped { pattern: Vec::new(), key: Vec::new() }, result, count)
    }

    pub fn shapeless(id: impl Into<String>, result: impl Into<String>, count: u32) -> Self {
        Self::of(id, RecipeKind::Shapeless { ingredients: Vec::new() }, result, count)
    }

    /// Cooking `ingredient` (100 % of a vanilla furnace: 0.1 experience,
    /// 200 ticks — see [`Self::experience`], [`Self::cook_time`]).
    pub fn cooking(
        id: impl Into<String>,
        method: Cooking,
        ingredient: impl Into<String>,
        result: impl Into<String>,
        count: u32,
    ) -> Self {
        let cook_time = match method {
            Cooking::Smelting => 200,
            Cooking::Blasting | Cooking::Smoking => 100,
            Cooking::Campfire => 600,
        };
        let kind = RecipeKind::Cooking { method, ingredient: vec![ingredient.into()], experience: 0.1, cook_time };
        Self::of(id, kind, result, count)
    }

    pub fn stonecutting(id: impl Into<String>, ingredient: impl Into<String>, result: impl Into<String>, count: u32) -> Self {
        Self::of(id, RecipeKind::Stonecutting { ingredient: vec![ingredient.into()] }, result, count)
    }

    pub fn smithing(
        id: impl Into<String>,
        template: impl Into<String>,
        base: impl Into<String>,
        addition: impl Into<String>,
        result: impl Into<String>,
    ) -> Self {
        let kind = RecipeKind::Smithing {
            template: vec![template.into()],
            base: vec![base.into()],
            addition: vec![addition.into()],
        };
        Self::of(id, kind, result, 1)
    }

    /// A row of a shaped recipe's pattern.
    pub fn row(mut self, pattern: impl Into<String>) -> Self {
        if let RecipeKind::Shaped { pattern: p, .. } = &mut self.kind {
            p.push(pattern.into());
        }
        self
    }

    /// What `symbol` stands for in a shaped recipe.
    pub fn key(self, symbol: char, item: impl Into<String>) -> Self {
        self.key_any(symbol, [item.into()])
    }

    /// `symbol` stands for any of `items`.
    pub fn key_any(mut self, symbol: char, items: impl IntoIterator<Item = impl Into<String>>) -> Self {
        if let RecipeKind::Shaped { key, .. } = &mut self.kind {
            key.push((symbol, items.into_iter().map(Into::into).collect()));
        }
        self
    }

    /// One more ingredient of a shapeless recipe.
    pub fn ingredient(self, item: impl Into<String>) -> Self {
        self.ingredient_any([item.into()])
    }

    /// One more ingredient of a shapeless recipe: any of `items`.
    pub fn ingredient_any(mut self, items: impl IntoIterator<Item = impl Into<String>>) -> Self {
        if let RecipeKind::Shapeless { ingredients } = &mut self.kind {
            ingredients.push(items.into_iter().map(Into::into).collect());
        }
        self
    }

    /// Experience a cooking recipe gives.
    pub fn experience(mut self, xp: f32) -> Self {
        if let RecipeKind::Cooking { experience, .. } = &mut self.kind {
            *experience = xp;
        }
        self
    }

    /// A cooking recipe's time in ticks.
    pub fn cook_time(mut self, ticks: u32) -> Self {
        if let RecipeKind::Cooking { cook_time, .. } = &mut self.kind {
            *cook_time = ticks;
        }
        self
    }

    pub fn group(mut self, group: impl Into<String>) -> Self {
        self.group = Some(group.into());
        self
    }

    pub fn category(mut self, category: impl Into<String>) -> Self {
        self.category = Some(category.into());
        self
    }

    /// The recipe in Yog's own format (the `.yrecipe` shape) — what
    /// `Registry::add_recipe` hands the runtime.
    pub fn to_data(&self) -> yog_data::Value {
        use yog_data::{Map, Value};
        let ing = |items: &[String]| match items {
            [one] => Value::from(one.as_str()),
            many => Value::List(many.iter().map(|i| Value::from(i.as_str())).collect()),
        };
        let mut m = Map::with_capacity(8);
        let ty = |m: &mut Map, t: &str| m.insert("type", Value::from(t));
        match &self.kind {
            RecipeKind::Shaped { pattern, key } => {
                ty(&mut m, "shaped");
                m.insert("pattern", Value::List(pattern.iter().map(|r| Value::from(r.as_str())).collect()));
                let mut keys = Map::with_capacity(key.len());
                for (c, items) in key {
                    keys.insert(c.to_string(), ing(items));
                }
                m.insert("key", Value::Map(keys));
            }
            RecipeKind::Shapeless { ingredients } => {
                ty(&mut m, "shapeless");
                m.insert("ingredients", Value::List(ingredients.iter().map(|i| ing(i)).collect()));
            }
            RecipeKind::Cooking { method, ingredient, experience, cook_time } => {
                ty(&mut m, method.word());
                m.insert("ingredient", ing(ingredient));
                // As written (`0.7`, not f32's 0.699999988…).
                m.insert("experience", Value::Float(experience.to_string().parse().unwrap_or(0.0)));
                m.insert("cook_time", Value::Int(*cook_time as i64));
            }
            RecipeKind::Stonecutting { ingredient } => {
                ty(&mut m, "stonecutting");
                m.insert("ingredient", ing(ingredient));
            }
            RecipeKind::Smithing { template, base, addition } => {
                ty(&mut m, "smithing");
                m.insert("template", ing(template));
                m.insert("base", ing(base));
                m.insert("addition", ing(addition));
            }
        }
        m.insert("result", Value::from(self.result.as_str()));
        if self.count != 1 {
            m.insert("count", Value::Int(self.count as i64));
        }
        if let Some(g) = &self.group {
            m.insert("group", Value::from(g.as_str()));
        }
        if let Some(c) = &self.category {
            m.insert("category", Value::from(c.as_str()));
        }
        Value::Map(m)
    }

    /// Split `namespace:name` from the recipe id.
    pub fn ns_name(&self) -> (&str, &str) {
        yog_ids::split(&self.id)
    }
}

// ── BookRecipe (like Patchouli's shapeless_book_recipe) ──────────────────────



// ── ItemModifier ─────────────────────────────────────────────────────────────



// ── StartupGrant ─────────────────────────────────────────────────────────────

/// Grant items/books to every player once when they first join.
/// This is the Yog-side replacement for `grant_patchi_book.json`.
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[derive(yog_data::Schema, Debug, Clone)]
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
}



// ── Tools and armor ─────────────────────────────────────────────────────────

/// An item as a tool or weapon.
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[derive(yog_data::Schema, Debug, Clone, PartialEq)]
pub struct ToolItem {
    /// `pickaxe`, `axe`, `shovel`, `hoe`, `sword`, or the mineable tag of
    /// a mod's kind of tool (`mymod:mineable/hammer`).
    pub kind: String,
    /// Its [`ToolMaterial`]'s id (`minecraft:iron`, `mymod:mythril`).
    pub material: String,
    /// Attack damage on top of the material's (an iron pickaxe: 1).
    pub attack_damage: f32,
    /// Attack speed change (a pickaxe: -2.8, a sword: -2.4).
    pub attack_speed: f32,
}

/// An item as a piece of armor.
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[derive(yog_data::Schema, Debug, Clone, PartialEq)]
pub struct ArmorPiece {
    /// `head`, `chest`, `legs` or `feet`.
    pub slot: String,
    /// Its [`ArmorMaterial`]'s id.
    pub material: String,
}

/// What tools are made of. The game's are built in
/// ([`ToolMaterial::built_ins`]); a mod declares its own in a `.ydef`
/// (`kind: "tool_material"`) or registers one
/// (`Registry::register_tool_material`).
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[derive(yog_data::Schema, Debug, Clone, PartialEq)]
pub struct ToolMaterial {
    pub id: String,
    /// Uses before it breaks.
    pub durability: u32,
    /// How fast it mines (wood 2 … netherite 9, gold 12).
    pub speed: f32,
    /// Attack damage it adds (wood 0 … netherite 4).
    pub attack_damage: f32,
    pub enchantability: u32,
    /// Its level on the tool ladder (`minecraft:iron`, `mymod:mythril`).
    pub level: String,
    /// What repairs it: an item id or `#tag`.
    pub repair: String,
    /// Minecraft 1.20's level number of `level` (wood 0 … netherite 4).
    #[data(default)]
    pub level_index: u32,
    /// Minecraft 1.21's tag of the blocks a tool of it can't mine for
    /// drops (`minecraft:incorrect_for_iron_tool`).
    #[data(default)]
    pub incorrect: String,
}

/// Defense points per armor piece.
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[derive(yog_data::Schema, Debug, Clone, Copy, PartialEq)]
pub struct ArmorDefense {
    pub head: u32,
    pub chest: u32,
    pub legs: u32,
    pub feet: u32,
}

/// What armor is made of. The game's are built in
/// ([`ArmorMaterial::built_ins`]); a mod declares its own in a `.ydef`
/// (`kind: "armor_material"`) or registers one.
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[derive(yog_data::Schema, Debug, Clone, PartialEq)]
pub struct ArmorMaterial {
    pub id: String,
    /// Multiplies each piece's base durability (helmet 11, chestplate 16,
    /// leggings 15, boots 13).
    pub durability: u32,
    pub defense: ArmorDefense,
    pub enchantability: u32,
    /// The sound event of putting it on.
    pub equip_sound: String,
    #[data(default)]
    pub toughness: f32,
    #[data(default)]
    pub knockback_resistance: f32,
    /// What repairs it: an item id or `#tag`.
    pub repair: String,
    /// Its look on a player: `<ns>:<name>` is
    /// `<ns>:textures/models/armor/<name>_layer_1.png` (and `_layer_2`).
    pub texture: String,
}

static BUILT_IN_TOOL_MATERIALS: std::sync::LazyLock<Vec<ToolMaterial>> = std::sync::LazyLock::new(|| {
    yog_data::from_text::<Vec<ToolMaterial>>(include_str!("../assets/tool_materials.ydata"))
        .unwrap_or_else(|e| panic!("yog-registry's built-in tool materials: {e}"))
        .value
});

static BUILT_IN_ARMOR_MATERIALS: std::sync::LazyLock<Vec<ArmorMaterial>> = std::sync::LazyLock::new(|| {
    yog_data::from_text::<Vec<ArmorMaterial>>(include_str!("../assets/armor_materials.ydata"))
        .unwrap_or_else(|e| panic!("yog-registry's built-in armor materials: {e}"))
        .value
});

/// One of `list` by short name or id (`iron` = `minecraft:iron`).
fn built_in<'a, T>(list: &'a [T], id: impl Fn(&T) -> &str, name: &str) -> Option<&'a T> {
    let name = yog_ids::qualify(name);
    list.iter().find(|x| id(x) == name)
}

impl ToolMaterial {
    /// The game's materials: wood, stone, iron, diamond, gold, netherite.
    pub fn built_ins() -> &'static [ToolMaterial] {
        &BUILT_IN_TOOL_MATERIALS
    }

    /// One of the game's, by short name or id.
    pub fn built_in(name: &str) -> Option<&'static ToolMaterial> {
        built_in(Self::built_ins(), |m| &m.id, name)
    }

    /// `id` with `like`'s level (one of the game's: `iron`), durability,
    /// speed, damage, enchantability and repair — change what differs.
    pub fn like(id: impl Into<String>, like: &str) -> Option<Self> {
        let mut m = Self::built_in(like)?.clone();
        m.id = id.into();
        Some(m)
    }
}

impl ArmorMaterial {
    /// The game's materials: leather, chainmail, iron, gold, diamond, turtle, netherite.
    pub fn built_ins() -> &'static [ArmorMaterial] {
        &BUILT_IN_ARMOR_MATERIALS
    }

    /// One of the game's, by short name or id.
    pub fn built_in(name: &str) -> Option<&'static ArmorMaterial> {
        built_in(Self::built_ins(), |m| &m.id, name)
    }
}

// ── World generation ────────────────────────────────────────────────────────

/// A placed feature (`mymod:mythril_ore`, the mod's datapack's) added to
/// biomes at a generation step — what an ore `.ydef` makes. Forge and
/// NeoForge read the biome modifiers `yog build` writes; Fabric's host adds
/// these through its biome API.
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[derive(yog_data::Schema, Debug, Clone, PartialEq)]
pub struct BiomeFeature {
    /// The placed feature's id.
    pub feature: String,
    /// The tag of biomes it goes in (`minecraft:is_overworld`).
    pub biomes: String,
    /// The generation step (`underground_ores`, `vegetal_decoration`, …).
    pub step: String,
}

// ── Sound groups ─────────────────────────────────────────────────────────────

/// What a block sounds like: the sound events it plays when broken,
/// stepped on, placed, hit while mining and fallen on. The game's groups
/// are built in ([`SoundGroup::built_in`]); a mod registers its own
/// (`Registry::register_sound_group`, or a `.ydef` of kind `sound_group`)
/// and names it in `BlockDef::sound`.
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[derive(yog_data::Schema, Debug, Clone, PartialEq)]
pub struct SoundGroup {
    pub id: String,
    /// Sound event ids (`minecraft:block.stone.break`, `mymod:crystal.break`).
    #[data(rename = "break")]
    pub break_sound: String,
    pub step: String,
    pub place: String,
    pub hit: String,
    pub fall: String,
    #[data(default = "one")]
    #[data(range = "0..=10")]
    pub volume: f32,
    #[data(default = "one")]
    #[data(range = "0..=10")]
    pub pitch: f32,
}

fn one() -> f32 {
    1.0
}

static BUILT_IN_SOUND_GROUPS: std::sync::LazyLock<Vec<SoundGroup>> = std::sync::LazyLock::new(|| {
    yog_data::from_text::<Vec<SoundGroup>>(include_str!("../assets/sound_groups.ydata"))
        .unwrap_or_else(|e| panic!("yog-registry's built-in sound groups: {e}"))
        .value
});

impl SoundGroup {
    /// `mymod:crystal`, its sounds `mymod:crystal.break`, `.step`, `.place`,
    /// `.hit`, `.fall`; volume and pitch 1.
    pub fn new(id: impl Into<String>) -> Self {
        let id = id.into();
        let ev = |s: &str| format!("{id}.{s}");
        Self { break_sound: ev("break"), step: ev("step"), place: ev("place"), hit: ev("hit"), fall: ev("fall"), volume: 1.0, pitch: 1.0, id }
    }

    /// The five sound events: break, step, place, hit, fall.
    pub fn sounds(mut self, break_sound: &str, step: &str, place: &str, hit: &str, fall: &str) -> Self {
        [self.break_sound, self.step, self.place, self.hit, self.fall] = [break_sound, step, place, hit, fall].map(String::from);
        self
    }

    pub fn volume(mut self, volume: f32) -> Self {
        self.volume = volume;
        self
    }

    pub fn pitch(mut self, pitch: f32) -> Self {
        self.pitch = pitch;
        self
    }

    /// The game's groups (`minecraft:stone`, `minecraft:amethyst`…, from
    /// Minecraft's `SoundType`), as data.
    pub fn built_ins() -> &'static [SoundGroup] {
        &BUILT_IN_SOUND_GROUPS
    }

    /// One of the game's groups, by short name or id (`stone` = `minecraft:stone`).
    pub fn built_in(name: &str) -> Option<&'static SoundGroup> {
        built_in(Self::built_ins(), |g| &g.id, name)
    }
}

// ── Blocks ───────────────────────────────────────────────────────────────────

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
    /// `Server::set_redstone_output`.
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
    /// with `Server::set_redstone_output` (0 until then).
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
        self.properties.push(BlockProperty { name: name.into(), values: values.iter().map(|v| v.to_string()).collect() });
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

// ── Entities ─────────────────────────────────────────────────────────────────

/// What kind of entity a type is.
#[derive(yog_data::Schema, Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum EntityKind {
    /// Not alive: a thing in the world (a scroll on a wall, a vehicle, a
    /// display) — no health, no AI.
    #[default]
    Object,
    /// Alive: health, attributes, hurt and death, but no AI of its own.
    Living,
    /// A creature with AI: `goals`, and walking about.
    Mob,
    /// Hangs on a block's side, as a painting: `width`×`height` whole
    /// blocks of wall (the item it's placed from may say another size —
    /// [`ItemDef::hangs`]); it stays while the blocks behind hold it, and
    /// breaking it, or losing its wall, drops the item it was placed from.
    Hanging,
}

/// An entity type to register (`.yentity`). Its look is a body model
/// (`model`, `.ymodel`) with clips by state; what it does beyond `goals` is
/// the mod's code (entity events, a tick of its own).
#[derive(yog_data::Schema, Debug, Clone)]
pub struct EntityDef {
    pub id: String,
    /// Its name (a text or a `@Set.key` resource); default: the id's lang key.
    pub name: Option<String>,
    #[data(default)]
    pub kind: EntityKind,
    /// Its spawn group (`MobCategory`): `misc`, `monster`, `creature`,
    /// `ambient`, `water_creature`, `water_ambient`, `underground_water_creature`.
    #[data(default = "misc")]
    pub category: String,
    /// Hitbox, blocks.
    pub width: f32,
    pub height: f32,
    /// Eye height, blocks; default 85% of the height.
    pub eye_height: Option<f32>,
    /// Lava and fire don't hurt it.
    #[data(default)]
    pub fire_immune: bool,
    /// It doesn't fall.
    #[data(default)]
    pub no_gravity: bool,
    /// Others push it (living things by default).
    pub pushable: Option<bool>,
    /// Others stand on and bump into it as into a block (a boat).
    #[data(default)]
    pub solid: bool,
    /// How far players see it, chunks.
    #[data(default = "eight")]
    pub tracking_range: u32,
    /// How often its movement is sent, ticks.
    #[data(default = "three")]
    pub update_interval: u32,
    /// Living and mob attributes by id: `max_health`, `movement_speed`,
    /// `attack_damage`, `follow_range`, `armor`, `knockback_resistance`… (the
    /// game's, `minecraft:` optional, or a mod's).
    #[data(default)]
    pub attributes: yog_data::Ordered<f64>,
    /// Its body model (`.ymodel` id); none: the mod draws it (`on_body`).
    pub model: Option<String>,
    /// Its clips (`.yanim` id).
    pub animations: Option<String>,
    /// Clip by state: `idle`, `walk`, `attack`, `hurt`, `death`, or the mod's
    /// own states (entity data `yog:state`).
    #[data(default)]
    pub clips: yog_data::Ordered<String>,
    /// Spawn egg colors `[base, spots]` (`0xRRGGBB`); none: no egg.
    pub spawn_egg: Option<[u32; 2]>,
    /// Mobs' AI goals, in priority order — the game's by name: `float`,
    /// `panic`, `melee_attack`, `wander`, `look_at_player`,
    /// `look_around`, `target_player`, `avoid_player`.
    #[data(default)]
    pub goals: Vec<String>,
    /// The mod handles each tick of every entity of this type
    /// (`yog_event::entity::game::Tick`, keyed by the type).
    #[data(default)]
    pub ticks: bool,
    /// A hanging thing's sound events when it's placed and when it breaks;
    /// default: a painting's.
    pub place_sound: Option<String>,
    pub break_sound: Option<String>,
}

fn eight() -> u32 {
    8
}

fn three() -> u32 {
    3
}

fn misc() -> String {
    "misc".into()
}

impl EntityDef {
    pub fn new(id: impl Into<String>, kind: EntityKind, width: f32, height: f32) -> Self {
        Self {
            id: id.into(),
            name: None,
            kind,
            category: misc(),
            width,
            height,
            eye_height: None,
            fire_immune: false,
            no_gravity: false,
            pushable: None,
            solid: false,
            tracking_range: eight(),
            update_interval: three(),
            attributes: yog_data::Ordered::default(),
            model: None,
            animations: None,
            clips: yog_data::Ordered::default(),
            spawn_egg: None,
            goals: Vec::new(),
            ticks: false,
            place_sound: None,
            break_sound: None,
        }
    }
}

#[cfg(test)]
mod registration_tests {
    #[test]
    fn a_hot_reload_sees_what_registration_changed() {
        let old = "block mymod:a 11\nblock mymod:b 22\nitem mymod:c 33\n";
        let new = "block mymod:a 11\nblock mymod:b 99\nitem mymod:d 44\n";
        let (added, removed, changed) = super::registration_changes(old, new);
        assert_eq!(added, ["item mymod:d"]);
        assert_eq!(removed, ["item mymod:c"]);
        assert_eq!(changed, ["block mymod:b"]);
        assert_eq!(super::registration_changes(old, old), (vec![], vec![], vec![]));
    }
}

#[cfg(test)]
mod material_tests {
    use super::*;

    #[test]
    fn the_games_materials_are_built_in() {
        assert_eq!(ToolMaterial::built_ins().len(), 6);
        let iron = ToolMaterial::built_in("iron").unwrap();
        assert_eq!((iron.durability, iron.level_index, iron.incorrect.as_str()), (250, 2, "minecraft:incorrect_for_iron_tool"));
        assert_eq!(ArmorMaterial::built_in("minecraft:netherite").unwrap().knockback_resistance, 0.1);
        assert_eq!(ArmorMaterial::built_in("diamond").unwrap().defense.chest, 8);
        let mythril = ToolMaterial::like("mymod:mythril", "diamond").unwrap();
        assert_eq!((mythril.id.as_str(), mythril.durability), ("mymod:mythril", 1561));
    }
}

#[cfg(test)]
mod sound_tests {
    use super::*;

    #[test]
    fn the_games_sound_groups_are_built_in() {
        assert!(SoundGroup::built_ins().len() > 100);
        let stone = SoundGroup::built_in("stone").unwrap();
        assert_eq!((stone.break_sound.as_str(), stone.volume), ("minecraft:block.stone.break", 1.0));
        assert_eq!(SoundGroup::built_in("minecraft:metal").unwrap().pitch, 1.5);
        assert!(SoundGroup::built_in("mymod:crystal").is_none());
    }

    #[test]
    fn a_mods_group_crosses_as_yog_data() {
        use yog_data::Schema;
        let g = SoundGroup::new("mymod:crystal").pitch(1.2);
        assert_eq!(g.fall, "mymod:crystal.fall");
        let back = yog_data::from_value::<SoundGroup>(&g.write()).unwrap().value;
        assert_eq!(back, g);
        assert_eq!(g.write().get("break").and_then(|v| v.as_str()), Some("mymod:crystal.break"));
    }
}

#[cfg(test)]
impl Recipe {
    fn to_json(&self) -> String {
        yog_data::json::to_json(&self.to_data()).to_string()
    }
}

#[cfg(test)]
mod recipe_tests {
    use super::*;

    #[test]
    fn writes_the_yog_format() {
        let js = |s: &str| serde_json::from_str::<serde_json::Value>(s).unwrap();
        let shaped = Recipe::shaped("m:a", "m:b", 2).row("D ").key('D', "#c:dusts").key_any('E', ["m:x", "m:y"]).group("g");
        assert_eq!(
            js(&shaped.to_json()),
            js(r##"{"type":"shaped","pattern":["D "],"key":{"D":"#c:dusts","E":["m:x","m:y"]},"result":"m:b","count":2,"group":"g"}"##)
        );
        assert_eq!(shaped.to_data().as_map().unwrap().keys().next(), Some("type"), "the type first");
        let blast = Recipe::cooking("m:a", Cooking::Blasting, "#c:ores", "m:b", 1).experience(0.7);
        assert_eq!(js(&blast.to_json()), js(r##"{"type":"blasting","ingredient":"#c:ores","experience":0.7,"cook_time":100,"result":"m:b"}"##));
        let smith = Recipe::smithing("m:a", "m:t", "m:base", "m:add", "m:out");
        assert_eq!(js(&smith.to_json())["template"], "m:t");
    }
}

#[cfg(test)]
mod json_tests {
    use super::*;

    /// Every `to_json` makes valid JSON, whatever the strings hold.
    #[test]
    fn json_survives_any_string() {
        let odd = "m:a\"b\\c\nd\te\u{1}";
        let parse = |j: String| serde_json::from_str::<serde_json::Value>(&j).unwrap_or_else(|e| panic!("{e}: {j}"));
        for r in [
            Recipe::shaped(odd, odd, 1).row("A").key('A', odd).group(odd).category(odd),
            Recipe::shapeless(odd, odd, 3).ingredient(odd).ingredient_any([odd, "m:x"]),
            Recipe::cooking(odd, Cooking::Smoking, odd, odd, 1).cook_time(5),
            Recipe::stonecutting(odd, odd, odd, 2),
            Recipe::smithing(odd, odd, odd, odd, odd),
        ] {
            let v = parse(r.to_json());
            assert_eq!(v["result"], odd, "the string comes back as it was");
        }
    }

    #[test]
    fn recipes_and_grants_name_their_parts() {
        let r = Recipe::cooking("mymod:a", Cooking::Campfire, "minecraft:beef", "minecraft:cooked_beef", 1);
        let v: serde_json::Value = serde_json::from_str(&r.to_json()).unwrap();
        assert_eq!(v["type"], "campfire_cooking");
        assert_eq!(v["cook_time"], 600, "the method's default time");
        assert_eq!(r.ns_name(), ("mymod", "a"));
        assert_eq!(Recipe::shaped("plain", "x", 1).ns_name().1, "plain");
    }
}
