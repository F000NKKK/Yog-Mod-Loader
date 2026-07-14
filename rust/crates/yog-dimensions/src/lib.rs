//! yog-dimensions — custom Minecraft dimension definitions for Yog mods.
//!
//! Declares a dimension's *type* (sky, lighting, physics, coordinate scale —
//! see [`YogDimensionTypeDef`]) via [`YogDimensionDef`], registered once at
//! mod-init time through `Registry::register_dimension` (in `yog-api`), the
//! same lifecycle window as blocks/items/recipes.
//!
//! Chunk *generation* is intentionally **not** a fixed preset/config here —
//! there's no single "noise" or "flat" style that covers every mod's needs,
//! and real generators often layer several algorithms together. Instead,
//! mods write their own generator as a plain closure registered via
//! `Registry::register_chunk_generator`, called once per chunk column with a
//! [`ChunkWriter`] handle the closure uses to place blocks however it likes
//! (composing whatever noise functions or logic it wants — that's the mod's
//! own code, not this crate's concern).

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
/// dimension. Serialized to JSON and sent across the ABI via
/// `Registry::register_dimension` (in `yog-api`), which the Java host parses
/// to register a `LevelStem`/`DimensionType` entry at mod-init time.
///
/// The actual `ServerLevel` (world data) is created lazily at runtime — see
/// the crate-level docs and `Registry::register_chunk_generator`.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct YogDimensionDef {
    pub id: String,
    pub dimension_type: YogDimensionTypeDef,
    /// Extra platform/metadata mods want the host to know about (arbitrary
    /// JSON-string values), for anything not modeled by `dimension_type`.
    pub extra: HashMap<String, String>,
}

impl Default for YogDimensionDef {
    fn default() -> Self {
        Self {
            id: String::new(),
            dimension_type: YogDimensionTypeDef::default(),
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

// ── Chunk writer handle ──────────────────────────────────────────────────────

/// Handle passed to a chunk-generator closure (see the crate-level docs),
/// wrapping the runtime's per-callback vtable — mirrors `GfxContext` in
/// `yog-gfx`. Valid only for the duration of the generator callback.
pub struct ChunkWriter(*const yog_abi::YogChunkWriterApi);

impl ChunkWriter {
    #[doc(hidden)]
    pub unsafe fn from_raw(raw: *const yog_abi::YogChunkWriterApi) -> Self {
        Self(raw)
    }

    #[inline]
    fn api(&self) -> &yog_abi::YogChunkWriterApi {
        unsafe { &*self.0 }
    }

    /// Chunk X coordinate (in chunks, not blocks).
    pub fn chunk_x(&self) -> i32 {
        self.api().chunk_x
    }

    /// Chunk Z coordinate (in chunks, not blocks).
    pub fn chunk_z(&self) -> i32 {
        self.api().chunk_z
    }

    /// Minimum build height (world Y) — matches the dimension type's `min_y`.
    pub fn min_y(&self) -> i32 {
        self.api().min_y
    }

    /// Total world height — matches the dimension type's `height`.
    pub fn height(&self) -> i32 {
        self.api().height
    }

    /// Set a block within this chunk column. `x`/`z` are **local** (0..16)
    /// chunk-relative coordinates; `y` is world-absolute (between `min_y()`
    /// and `min_y() + height()`).
    pub fn set_block(&self, x: i32, y: i32, z: i32, block_id: &str) -> bool {
        let api = self.api();
        unsafe { (api.set_block)(api.ctx, x, y, z, yog_abi::YogStr::from_str(block_id)) }
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
            .with_extra("note", "test");

        let json = def.to_json();
        let back: YogDimensionDef = serde_json::from_str(&json).expect("valid json");
        assert_eq!(back.id, "mymod:my_dim");
        assert!(back.dimension_type.ultrawarm);
        assert_eq!(back.dimension_type.effects, "nether");
        assert_eq!(back.extra.get("note").map(String::as_str), Some("test"));
    }
}
