//! The game's worldgen data this module writes: `data/<ns>/worldgen/…`,
//! and the biome modifiers Forge and NeoForge add features to biomes with
//! (`data/<ns>/<loader>/biome_modifier/…`).

/// Its folders of features: what one is, and where it's placed.
pub(super) const CONFIGURED_FEATURE: &str = "configured_feature";
pub(super) const PLACED_FEATURE: &str = "placed_feature";
/// The kinds of feature a `.ydef` makes.
pub(super) const TREE: &str = yog_ids::id!(yog_ids::ns::MINECRAFT, "tree");
pub(super) const ORE: &str = yog_ids::id!(yog_ids::ns::MINECRAFT, "ore");
/// What blocks a feature places: one, or some by weight; one by rules.
pub(super) const SIMPLE_STATE_PROVIDER: &str =
    yog_ids::id!(yog_ids::ns::MINECRAFT, "simple_state_provider");
pub(super) const WEIGHTED_STATE_PROVIDER: &str =
    yog_ids::id!(yog_ids::ns::MINECRAFT, "weighted_state_provider");
pub(super) const RULE_BASED_STATE_PROVIDER: &str =
    yog_ids::id!(yog_ids::ns::MINECRAFT, "rule_based_state_provider");
/// Block predicates: not, in a tag.
pub(super) const NOT: &str = yog_ids::id!(yog_ids::ns::MINECRAFT, "not");
pub(super) const MATCHING_BLOCK_TAG: &str =
    yog_ids::id!(yog_ids::ns::MINECRAFT, "matching_block_tag");
/// What a tree's trunk doesn't put dirt over.
pub(super) const CANNOT_REPLACE_BELOW_TREE_TRUNK: &str =
    yog_ids::id!(yog_ids::ns::MINECRAFT, "cannot_replace_below_tree_trunk");
/// A tree's size check.
pub(super) const TWO_LAYERS_FEATURE_SIZE: &str =
    yog_ids::id!(yog_ids::ns::MINECRAFT, "two_layers_feature_size");
/// What a tree grows on when it doesn't say.
pub(super) const DIRT: &str = yog_ids::id!(yog_ids::ns::MINECRAFT, "dirt");
/// A placer's id: `minecraft:<shape>_<part>_placer`.
pub(super) const TRUNK: &str = "trunk";
pub(super) const FOLIAGE: &str = "foliage";
pub(super) const PLACER: &str = "placer";
/// What an ore replaces: blocks of a tag, or one block.
pub(super) const TAG_MATCH: &str = yog_ids::id!(yog_ids::ns::MINECRAFT, "tag_match");
pub(super) const BLOCK_MATCH: &str = yog_ids::id!(yog_ids::ns::MINECRAFT, "block_match");
/// Heights an ore is spread over: evenly, or most in the middle.
pub(super) const UNIFORM: &str = yog_ids::id!(yog_ids::ns::MINECRAFT, "uniform");
pub(super) const TRAPEZOID: &str = yog_ids::id!(yog_ids::ns::MINECRAFT, "trapezoid");
/// Where a placed feature goes: so many a chunk, spread in it, at
/// heights, in the biomes that have it.
pub(super) const COUNT: &str = yog_ids::id!(yog_ids::ns::MINECRAFT, "count");
pub(super) const IN_SQUARE: &str = yog_ids::id!(yog_ids::ns::MINECRAFT, "in_square");
pub(super) const HEIGHT_RANGE: &str = yog_ids::id!(yog_ids::ns::MINECRAFT, "height_range");
pub(super) const BIOME: &str = yog_ids::id!(yog_ids::ns::MINECRAFT, "biome");
/// The biomes an ore is in when it doesn't say.
pub(super) const IS_OVERWORLD: &str = yog_ids::id!(yog_ids::ns::MINECRAFT, "is_overworld");
/// The generation step ores are placed in.
pub(super) const UNDERGROUND_ORES: &str = "underground_ores";

pub(super) const BIOME_MODIFIER_DIR: &str = "biome_modifier";
pub(super) const BIOME_MODIFIER_FORGE_ADD_FEATURES: &str =
    yog_ids::id!(yog_ids::loader::FORGE, "add_features");
pub(super) const BIOME_MODIFIER_NEOFORGE_ADD_FEATURES: &str =
    yog_ids::id!(yog_ids::loader::NEOFORGE, "add_features");
