//! yog-dimensions — custom Minecraft dimension definitions for Yog mods.
//!
//! Declares a dimension's *type* (sky, lighting, physics, coordinate scale —
//! see [`YogDimensionTypeDef`]) via [`YogDimensionDef`], registered once at
//! mod-init time through `Registry::register_dimension` (in `yog-api`), the
//! same lifecycle window as blocks/items/recipes.
//!
//! **`id` isn't limited to new custom dimensions** — passing a vanilla id
//! (`"minecraft:overworld"`, `"minecraft:the_nether"`, `"minecraft:the_end"`)
//! patches that dimension's type/generator instead of creating a new one.
//! This is how a mod replaces or tweaks the overworld's terrain, not just
//! adds side dimensions.
//!
//! Chunk *generation* is intentionally **not** a fixed preset/config here —
//! there's no single "noise" or "flat" style that covers every mod's needs,
//! and real generators often layer several algorithms together. Instead,
//! mods write their own generator as a plain closure and register it under a
//! **named, reusable generator type** via `Registry::register_chunk_generator`
//! (e.g. `"mymod:custom_terrain"`) — called once per chunk column with the
//! [`GeneratedChunk`] the closure places blocks in however it likes
//! (composing whatever noise functions or logic it wants — that's the mod's
//! own code, not this crate's concern).
//!
//! Because generator types are named and independent of any one dimension, a
//! [`YogDimensionDef`] just *references* one by id via
//! [`YogDimensionDef::generator_type`] — the same generator type can back
//! several dimensions, and (like Biomes O'Plenty's world-type presets) the
//! platform host can list every registered generator type as a selectable
//! option on the world-creation screen, letting a player pick which
//! registered generator the overworld (or a custom dimension) uses for a
//! new world.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

// ── Dimension type ───────────────────────────────────────────────────────────

/// Properties of a dimension type — mirrors Java's `YogDimensionType`.
///
/// All fields default to vanilla-overworld-like values so mods only need to
/// override what actually differs. `#[serde(default)]` on every field keeps
/// this forward-compatible: older mod JSON still deserializes on a runtime
/// that's added new fields.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct YogDimensionTypeDef {
    pub min_y: i32,
    pub height: i32,
    pub logical_height: i32,
    pub has_sky_light: bool,
    pub sky_light_updates: bool,
    pub ambient_light: f32,
    pub coordinate_scale: f32,
    pub ultrawarm: bool,
    pub beds_explode: bool,
    pub respawn_anchors_explode: bool,
    pub piglin_safe: bool,
    pub natural: bool,
    pub has_ceiling: bool,
    pub effects: String,
    pub has_sky: bool,
    pub has_clouds: bool,
    pub cloud_height: f64,
    pub has_fog: bool,
    pub fog_color: Option<i32>,
    pub sky_color: Option<i32>,
    pub water_color: Option<i32>,
    pub water_fog_color: Option<i32>,
}

impl Default for YogDimensionTypeDef {
    fn default() -> Self {
        Self {
            min_y: -64,
            height: 384,
            logical_height: 384,
            has_sky_light: true,
            sky_light_updates: true,
            ambient_light: 0.0,
            coordinate_scale: 1.0,
            ultrawarm: false,
            beds_explode: false,
            respawn_anchors_explode: false,
            piglin_safe: false,
            natural: true,
            has_ceiling: false,
            effects: "overworld".to_owned(),
            has_sky: true,
            has_clouds: true,
            cloud_height: 192.0,
            has_fog: false,
            fog_color: None,
            sky_color: None,
            water_color: None,
            water_fog_color: None,
        }
    }
}

impl YogDimensionTypeDef {
    // ── Chained setters — start from `YogDimensionTypeDef::default()` and
    // override only what differs from the overworld-like baseline. ─────────

    pub fn min_y(mut self, v: i32) -> Self {
        self.min_y = v;
        self
    }
    pub fn height(mut self, v: i32) -> Self {
        self.height = v;
        self.logical_height = v;
        self
    }
    pub fn logical_height(mut self, v: i32) -> Self {
        self.logical_height = v;
        self
    }
    pub fn has_sky_light(mut self, v: bool) -> Self {
        self.has_sky_light = v;
        self
    }
    pub fn sky_light_updates(mut self, v: bool) -> Self {
        self.sky_light_updates = v;
        self
    }
    pub fn ambient_light(mut self, v: f32) -> Self {
        self.ambient_light = v;
        self
    }
    pub fn coordinate_scale(mut self, v: f32) -> Self {
        self.coordinate_scale = v;
        self
    }
    pub fn ultrawarm(mut self, v: bool) -> Self {
        self.ultrawarm = v;
        self
    }
    pub fn beds_explode(mut self, v: bool) -> Self {
        self.beds_explode = v;
        self
    }
    pub fn respawn_anchors_explode(mut self, v: bool) -> Self {
        self.respawn_anchors_explode = v;
        self
    }
    pub fn piglin_safe(mut self, v: bool) -> Self {
        self.piglin_safe = v;
        self
    }
    pub fn natural(mut self, v: bool) -> Self {
        self.natural = v;
        self
    }
    pub fn has_ceiling(mut self, v: bool) -> Self {
        self.has_ceiling = v;
        self
    }
    pub fn effects(mut self, v: impl Into<String>) -> Self {
        self.effects = v.into();
        self
    }
    pub fn has_sky(mut self, v: bool) -> Self {
        self.has_sky = v;
        self
    }
    pub fn has_clouds(mut self, v: bool) -> Self {
        self.has_clouds = v;
        self
    }
    pub fn cloud_height(mut self, v: f64) -> Self {
        self.cloud_height = v;
        self
    }
    pub fn has_fog(mut self, v: bool) -> Self {
        self.has_fog = v;
        self
    }
    pub fn fog_color(mut self, v: i32) -> Self {
        self.fog_color = Some(v);
        self
    }
    pub fn sky_color(mut self, v: i32) -> Self {
        self.sky_color = Some(v);
        self
    }
    pub fn water_color(mut self, v: i32) -> Self {
        self.water_color = Some(v);
        self
    }
    pub fn water_fog_color(mut self, v: i32) -> Self {
        self.water_fog_color = Some(v);
        self
    }
}

// ── Dimension definition ─────────────────────────────────────────────────────

/// A dimension definition — what a mod registers to create a custom
/// dimension, or to patch an existing (including vanilla) one. Serialized
/// to JSON and sent across the ABI via `Registry::register_dimension` (in
/// `yog-api`), which the Java host parses to register (or patch) a
/// `LevelStem`/`DimensionType` entry at mod-init time.
///
/// The actual `ServerLevel` (world data) is created lazily at runtime — see
/// the crate-level docs and `Registry::register_chunk_generator`.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct YogDimensionDef {
    /// Registry id. A new id (e.g. `"mymod:my_dim"`) creates a custom
    /// dimension; a vanilla id (`"minecraft:overworld"`, etc.) patches that
    /// dimension's type/generator instead.
    pub id: String,
    pub dimension_type: YogDimensionTypeDef,
    /// Id of a generator type registered via `Registry::register_chunk_generator`
    /// (e.g. `"mymod:custom_terrain"`) that this dimension should use. `None`
    /// keeps the dimension's existing/vanilla generator.
    pub generator_type: Option<String>,
    /// The one biome the dimension is made of (`"minecraft:plains"`): its
    /// mobs, its features, the structures that may start in it.
    pub biome: String,
    /// Structures (villages, …) may generate. Only for dimensions with a
    /// registered generator.
    pub structures: bool,
    /// The biome's features (trees, flowers, ores) are placed. Only for
    /// dimensions with a registered generator.
    pub features: bool,
    /// Mobs spawn naturally (a registered generator also offers none to
    /// the spawner when off).
    pub mob_spawning: bool,
    /// Extra platform/metadata mods want the host to know about (arbitrary
    /// JSON-string values), for anything not modeled by `dimension_type`.
    pub extra: HashMap<String, String>,
}

impl Default for YogDimensionDef {
    fn default() -> Self {
        Self {
            id: String::new(),
            dimension_type: YogDimensionTypeDef::default(),
            generator_type: None,
            biome: "minecraft:plains".into(),
            structures: true,
            features: true,
            mob_spawning: true,
            extra: HashMap::new(),
        }
    }
}

impl YogDimensionDef {
    /// Start a new definition with default (overworld-like) type properties.
    pub fn new(id: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            ..Self::default()
        }
    }

    /// Replace the dimension type properties.
    pub fn dimension_type(mut self, dimension_type: YogDimensionTypeDef) -> Self {
        self.dimension_type = dimension_type;
        self
    }

    /// Use the generator type registered under this id (see
    /// `Registry::register_chunk_generator`).
    pub fn generator_type(mut self, generator_type_id: impl Into<String>) -> Self {
        self.generator_type = Some(generator_type_id.into());
        self
    }

    /// The biome the dimension is made of.
    pub fn biome(mut self, biome: impl Into<String>) -> Self {
        self.biome = biome.into();
        self
    }

    /// Whether structures generate (default: yes).
    pub fn structures(mut self, on: bool) -> Self {
        self.structures = on;
        self
    }

    /// Whether the biome's features are placed (default: yes).
    pub fn features(mut self, on: bool) -> Self {
        self.features = on;
        self
    }

    /// Whether mobs spawn (default: yes).
    pub fn mob_spawning(mut self, on: bool) -> Self {
        self.mob_spawning = on;
        self
    }

    /// Set an extra metadata property.
    pub fn with_extra(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.extra.insert(key.into(), value.into());
        self
    }

    /// Serialize to the JSON wire format sent to the Java host.
    pub fn to_json(&self) -> String {
        serde_json::to_string(self).expect("YogDimensionDef serializes infallibly")
    }
}

// ── Generated chunks ─────────────────────────────────────────────────────────

/// Blocks across a chunk column, in x and z.
pub const CHUNK_WIDTH: i32 = 16;

/// A chunk column a generator fills (the `world/generation/chunk` event,
/// keyed by `generator`): where it is, and every block placed so far — a
/// palette and a cell per block, like the game's chunk sections, so the host
/// sets them a section at a time. Several handlers fill one column in turn
/// (terrain, then a mod's ores).
#[derive(yog_data::Schema, Debug, Clone, Default, PartialEq)]
pub struct GeneratedChunk {
    /// The generator type (`mymod:custom_terrain`).
    pub generator: String,
    /// In chunks, not blocks.
    pub chunk_x: i32,
    pub chunk_z: i32,
    /// The dimension type's `min_y` and `height`.
    pub min_y: i32,
    pub height: i32,
    /// The block ids the cells name: cell value `n` is `palette[n - 1]`.
    #[data(default)]
    pub palette: Vec<String>,
    /// One little-endian `u16` per block, Y then Z then X ([`CELL`] each;
    /// `0`: nothing placed); empty until the first block.
    #[data(with = "yog_data::with::Raw")]
    pub cells: Vec<u8>,
}

/// Bytes per cell of [`GeneratedChunk::cells`].
pub const CELL: usize = std::mem::size_of::<u16>();

impl GeneratedChunk {
    /// The cell of local `x`/`z` (0..16), world `y`; `None` outside the column.
    fn cell(&self, x: i32, y: i32, z: i32) -> Option<usize> {
        let inside = (0..CHUNK_WIDTH).contains(&x) && (0..CHUNK_WIDTH).contains(&z) && (self.min_y..self.min_y + self.height).contains(&y);
        inside.then(|| (((y - self.min_y) * CHUNK_WIDTH + z) * CHUNK_WIDTH + x) as usize)
    }

    /// Places `block_id` at local `x`/`z` (0..16), world `y` (between
    /// `min_y` and `min_y + height`) over whatever was there; `false`
    /// outside the column (or past `u16::MAX` distinct blocks).
    pub fn set_block(&mut self, x: i32, y: i32, z: i32, block_id: &str) -> bool {
        let Some(cell) = self.cell(x, y, z) else { return false };
        let index = match self.palette.iter().position(|b| b == block_id) {
            Some(i) => i,
            None => {
                self.palette.push(block_id.to_owned());
                self.palette.len() - 1
            }
        };
        let Ok(value) = u16::try_from(index + 1) else { return false };
        if self.cells.is_empty() {
            self.cells = vec![0; (CHUNK_WIDTH * CHUNK_WIDTH * self.height.max(0)) as usize * CELL];
        }
        self.cells[cell * CELL..(cell + 1) * CELL].copy_from_slice(&value.to_le_bytes());
        true
    }

    /// The block placed at local `x`/`z`, world `y`, if any.
    pub fn block(&self, x: i32, y: i32, z: i32) -> Option<&str> {
        let cell = self.cell(x, y, z)?;
        let bytes = self.cells.get(cell * CELL..(cell + 1) * CELL)?;
        let value = u16::from_le_bytes([bytes[0], bytes[1]]) as usize;
        self.palette.get(value.checked_sub(1)?).map(String::as_str)
    }

    /// Every block placed, as (local x, world y, local z, id), Y then Z then X.
    pub fn placed(&self) -> impl Iterator<Item = (i32, i32, i32, &str)> {
        let width = CHUNK_WIDTH as usize;
        self.cells.as_chunks::<CELL>().0.iter().enumerate().filter_map(move |(cell, b)| {
            let value = u16::from_le_bytes(*b) as usize;
            let id = self.palette.get(value.checked_sub(1)?)?;
            let (x, z, y) = (cell % width, cell / width % width, cell / (width * width));
            Some((x as i32, y as i32 + self.min_y, z as i32, id.as_str()))
        })
    }
}

#[cfg(test)]
mod chunk_tests {
    use super::*;

    #[test]
    fn placed_blocks_share_a_palette_and_stay_in_the_column() {
        let mut chunk = GeneratedChunk { min_y: -64, height: 384, ..Default::default() };
        assert!(chunk.cells.is_empty(), "nothing allocated before the first block");
        assert!(chunk.set_block(0, -64, 15, "minecraft:bedrock"));
        assert!(chunk.set_block(1, 0, 1, "minecraft:stone"));
        assert!(chunk.set_block(1, 0, 1, "minecraft:diamond_ore"), "a later block replaces an earlier one");
        assert!(chunk.set_block(2, 0, 2, "minecraft:stone"));
        assert!(!chunk.set_block(16, 0, 0, "minecraft:stone"), "x past the column");
        assert!(!chunk.set_block(0, 320, 0, "minecraft:stone"), "y past the top");
        assert_eq!(chunk.palette, ["minecraft:bedrock", "minecraft:stone", "minecraft:diamond_ore"]);
        assert_eq!(chunk.block(1, 0, 1), Some("minecraft:diamond_ore"));
        assert_eq!(chunk.block(3, 0, 3), None);
        let placed: Vec<_> = chunk.placed().collect();
        assert_eq!(placed, [(0, -64, 15, "minecraft:bedrock"), (1, 0, 1, "minecraft:diamond_ore"), (2, 0, 2, "minecraft:stone")]);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dimension_def_to_json_roundtrips_through_serde() {
        let def = YogDimensionDef::new("mymod:my_dim")
            .dimension_type(
                YogDimensionTypeDef::default()
                    .ultrawarm(true)
                    .effects("nether"),
            )
            .generator_type("mymod:custom_terrain")
            .with_extra("note", "test");

        let json = def.to_json();
        let back: YogDimensionDef = serde_json::from_str(&json).expect("valid json");
        assert_eq!(back.id, "mymod:my_dim");
        assert!(back.dimension_type.ultrawarm);
        assert_eq!(back.dimension_type.effects, "nether");
        assert_eq!(back.generator_type.as_deref(), Some("mymod:custom_terrain"));
        assert_eq!(back.extra.get("note").map(String::as_str), Some("test"));
    }

    #[test]
    fn dimension_def_can_target_a_vanilla_id_to_patch_it() {
        let def =
            YogDimensionDef::new("minecraft:overworld").generator_type("mymod:custom_terrain");
        assert_eq!(def.id, "minecraft:overworld");
        assert_eq!(def.generator_type.as_deref(), Some("mymod:custom_terrain"));
    }
}
