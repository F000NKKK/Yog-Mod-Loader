//! One checked `.yblock` / `.yitem`: what the generated code needs of it.

use yog_ids::Format;

#[derive(Debug, Clone, Default)]
pub(in crate::content) struct BlockProps {
    pub(in crate::content) hardness: Option<f64>,
    pub(in crate::content) resistance: Option<f64>,
    pub(in crate::content) sound: Option<String>,
    pub(in crate::content) light: u8,
    /// The tool tags it joins (`minecraft:mineable/pickaxe`, a mod's own).
    pub(in crate::content) tools: Vec<String>,
    /// The tool level it needs (`minecraft:iron`, a mod's `.ydef` level) and
    /// the line that says so.
    pub(in crate::content) tool_level: Option<(String, usize)>,
    pub(in crate::content) requires_tool: bool,
    pub(in crate::content) no_collision: bool,
    pub(in crate::content) slipperiness: Option<f64>,
    /// `None` = full cube.
    pub(in crate::content) shape: Option<[f64; 6]>,
    pub(in crate::content) connect_groups: Vec<String>,
    pub(in crate::content) connects: bool,
    /// `""`, `"horizontal"` or `"all"`.
    pub(in crate::content) facing: String,
    /// `"toward"` or `"away"`.
    pub(in crate::content) placement: String,
    pub(in crate::content) attach: bool,
    pub(in crate::content) waterlogged: bool,
    /// Its own properties: name, values.
    pub(in crate::content) properties: Vec<(String, Vec<String>)>,
    /// Yes/no properties: kept on by redstone; read by a comparator.
    pub(in crate::content) powered: Option<String>,
    pub(in crate::content) comparator: Option<String>,
    /// Hoppers and droppers can put items into it.
    pub(in crate::content) accepts_items: bool,
    /// The inventory screen it opens (a `.yui` `<Screen>` with `<Slot>`s).
    pub(in crate::content) inventory: Option<String>,
    /// Gives off redstone power, set by the mod per position.
    pub(in crate::content) redstone: bool,
    /// Doesn't hide the faces behind it or stop light.
    pub(in crate::content) no_occlusion: bool,
    /// One of the game's kinds of block (`stairs`…); `""`: Yog's own.
    pub(in crate::content) kind: String,
    /// What a kind is made from, its set, a button's press and who sets it off.
    pub(in crate::content) base: Option<String>,
    pub(in crate::content) set: Option<String>,
    pub(in crate::content) press_ticks: Option<u32>,
    pub(in crate::content) mobs_only: bool,
}

#[derive(Debug, Clone, Default)]
pub(in crate::content) struct ItemProps {
    pub(in crate::content) stack_size: Option<u8>,
    pub(in crate::content) durability: u32,
    pub(in crate::content) fire_resistant: bool,
    pub(in crate::content) fuel_ticks: u32,
    /// (nutrition, saturation, always_edible)
    pub(in crate::content) food: Option<(u32, f64, bool)>,
    /// (kind, material, attack damage, attack speed)
    pub(in crate::content) tool: Option<(String, String, Option<f32>, Option<f32>)>,
    /// (slot, material)
    pub(in crate::content) armor: Option<(yog_registry::ArmorSlot, String)>,
    /// Worn in this slot without being armor.
    pub(in crate::content) wearable: Option<yog_registry::ArmorSlot>,
    /// The hanging entity it places, and its size in blocks.
    pub(in crate::content) hangs: Option<(String, Option<[u32; 2]>)>,
}

/// One checked content file — what the Rust code needs.
#[derive(Debug, Clone)]
pub(in crate::content) struct Entry {
    pub(in crate::content) kind: Format,
    pub(in crate::content) id: String,
    pub(in crate::content) file: String,
    pub(in crate::content) auto: bool,
    /// Mods it needs (`requires`): registered only when they're all loaded.
    pub(in crate::content) requires: Vec<String>,
    pub(in crate::content) block: BlockProps,
    pub(in crate::content) item: ItemProps,
    pub(in crate::content) potion: Option<yog_registry::PotionDef>,
}
