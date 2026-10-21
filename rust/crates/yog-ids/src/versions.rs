//! Versions two parts of Yog must agree on.

/// The `.yui` code yog writes (`yog build`) and yog-kit runs: generated
/// code checks it at compile time (`yog_kit::__codegen::require`).
pub const UI_CODEGEN: u32 = 3;

/// The game versions NeoForge reads Forge's biome modifiers on, and those
/// it reads its own on: the version folders of the overlays the build
/// writes them in, which the hosts pick by.
pub const NEOFORGE_READS_FORGE: &str = "1.20";
pub const NEOFORGE_READS_OWN: &str = "1.21";
