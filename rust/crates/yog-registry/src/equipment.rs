//! Tools and armor: their items, and the materials they're made of (the game's built in).

#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};
use yog_ids::tool_kind::{AXE, HOE, PICKAXE, SHOVEL, SWORD};

/// An item as a tool or weapon.
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[derive(yog_data::Schema, Debug, Clone, PartialEq)]
pub struct ToolItem {
    /// `pickaxe`, `axe`, `shovel`, `hoe`, `sword`, or the mineable tag of
    /// a mod's kind of tool (`mymod:mineable/hammer`).
    pub kind: String,
    /// Its [`ToolMaterial`]'s id (`minecraft:iron`, `mymod:mythril`).
    pub material: String,
    /// Attack damage on top of the material's; none: the game's tool's
    /// ([`ToolMaterial::game_tool`]), as the running version has it.
    pub attack_damage: Option<f32>,
    /// Attack speed change; none: the game's tool's.
    pub attack_speed: Option<f32>,
}

/// The game's kinds of tool that mine.
pub const DIGGERS: [&str; 4] = [PICKAXE, AXE, SHOVEL, HOE];
/// The game's kinds of tool: its diggers and the sword.
pub const TOOL_KINDS: [&str; 5] = [PICKAXE, AXE, SHOVEL, HOE, SWORD];

/// A mod's material takes its tools' attack from the game's tools of this
/// one; a mod's kind of tool, from the game's of this kind.
pub const REFERENCE_MATERIAL: &str = yog_ids::id!(yog_ids::ns::MINECRAFT, "iron");
pub const REFERENCE_KIND: &str = PICKAXE;

/// The game's tool a tool takes its attack from when it gives none: its
/// item id, and the attack damage its material adds (what the kind adds is
/// the rest).
#[derive(yog_data::Schema, Debug, Clone, PartialEq)]
pub struct GameTool {
    pub item: String,
    pub material_damage: f32,
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
    /// What the game's tools of it are named by (`wooden_`: `minecraft:wooden_axe`);
    /// empty for a mod's own.
    #[data(default)]
    pub game_tools: String,
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

static BUILT_IN_TOOL_MATERIALS: yog_sync::LazyLock<Vec<ToolMaterial>> =
    yog_sync::LazyLock::new(|| {
        yog_data::built_in::<Vec<ToolMaterial>>(
            "yog-registry's built-in tool materials",
            include_str!("../assets/tool_materials.ydata"),
        )
    });

static BUILT_IN_ARMOR_MATERIALS: yog_sync::LazyLock<Vec<ArmorMaterial>> =
    yog_sync::LazyLock::new(|| {
        yog_data::built_in::<Vec<ArmorMaterial>>(
            "yog-registry's built-in armor materials",
            include_str!("../assets/armor_materials.ydata"),
        )
    });

/// One of `list` by short name or id (`iron` = `minecraft:iron`).
pub(crate) fn built_in<'a, T>(list: &'a [T], id: impl Fn(&T) -> &str, name: &str) -> Option<&'a T> {
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

    /// The game's tool of `kind` (a mod's kind: [`REFERENCE_KIND`]) made of
    /// this material, or — a mod's — of [`REFERENCE_MATERIAL`].
    pub fn game_tool(&self, kind: &str) -> Option<GameTool> {
        let made = if self.game_tools.is_empty() {
            Self::built_in(REFERENCE_MATERIAL)?
        } else {
            Self::built_ins().iter().find(|m| m.game_tools == self.game_tools)?
        };
        let kind = if TOOL_KINDS.contains(&kind) { kind } else { REFERENCE_KIND };
        Some(GameTool {
            item: yog_ids::qualify(&format!("{}{kind}", made.game_tools)),
            material_damage: made.attack_damage,
        })
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
