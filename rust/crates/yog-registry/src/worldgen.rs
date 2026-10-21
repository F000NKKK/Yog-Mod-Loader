//! World generation: features a mod puts in biomes.

/// A placed feature (`mymod:mythril_ore`, the mod's datapack's) added to
/// biomes at a generation step — what an ore `.ydef` makes. Forge and
/// NeoForge read the biome modifiers `yog build` writes; Fabric's host adds
/// these through its biome API.
#[derive(yog_data::Schema, Debug, Clone, PartialEq)]
pub struct BiomeFeature {
    /// The placed feature's id.
    pub feature: String,
    /// The tag of biomes it goes in (`minecraft:is_overworld`).
    pub biomes: String,
    /// The generation step (`underground_ores`, `vegetal_decoration`, …).
    pub step: String,
}
