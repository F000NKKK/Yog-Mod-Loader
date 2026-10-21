//! The game's data the build writes from `.ydef`s: worldgen features,
//! the loaders' biome modifiers, advancements.

names! {
    /// The loaders a pack's overlays are for: `when/<loader>/<version>/…`.
    loader {
        FABRIC = "fabric",
        FORGE = "forge",
        NEOFORGE = "neoforge",
    }

    /// The game's data folders renamed in 1.21 (`data/<ns>/<dir>/`): before,
    /// each was its name and [`crate::data_layout::PLURAL`].
    renamed_data_dir {
        ADVANCEMENT = crate::kind::ADVANCEMENT,
        FUNCTION = "function",
        ITEM_MODIFIER = "item_modifier",
        LOOT_TABLE = crate::kind::LOOT_TABLE,
        PREDICATE = "predicate",
        RECIPE = crate::kind::RECIPE,
        STRUCTURE = "structure",
    }

    /// The tag folders renamed in 1.21 (`data/<ns>/tags/<dir>/`), likewise.
    renamed_tag_dir {
        BLOCK = crate::kind::BLOCK,
        ENTITY_TYPE = crate::concatcp!(crate::kind::ENTITY, crate::sep::WORD, crate::word::TYPE),
        FLUID = crate::kind::FLUID,
        FUNCTION = "function",
        GAME_EVENT = "game_event",
        ITEM = crate::kind::ITEM,
    }

    /// How data folders were named before 1.21.
    data_layout {
        /// What an old folder's name adds: `recipes`.
        PLURAL = "s",
    }

    /// The game's kinds of recipe, as `.yrecipe` files name them (`type`),
    /// in the order yog-registry's data has them.
    recipe_kind {
        SHAPED = "shaped",
        SHAPELESS = "shapeless",
        SMELTING = "smelting",
        BLASTING = "blasting",
        SMOKING = "smoking",
        CAMPFIRE_COOKING = "campfire_cooking",
        STONECUTTING = "stonecutting",
        SMITHING = "smithing",
        BREWING = "brewing",
    }

    /// A recipe's fields as Yog writes it (`.yrecipe`, and what the hosts get).
    recipe_field {
        TYPE = crate::word::TYPE,
        RESULT = "result",
        COUNT = crate::word::COUNT,
        /// A mod's kind: its slots and values, by name.
        SLOTS = "slots",
        VALUES = crate::word::VALUES,
        /// A grid: rows of key characters, and what each character is.
        PATTERN = "pattern",
        KEY = "key",
        INGREDIENTS = "ingredients",
        INGREDIENT = "ingredient",
        TEMPLATE = "template",
        BASE = "base",
        ADDITION = "addition",
        EXPERIENCE = "experience",
        COOK_TIME = "cook_time",
        GROUP = "group",
        CATEGORY = "category",
    }

    /// The game's recipe JSON: the types Yog's kinds become, and its own fields.
    game_recipe {
        CRAFTING_SHAPED = crate::id!(crate::ns::MINECRAFT, "crafting_shaped"),
        CRAFTING_SHAPELESS = crate::id!(crate::ns::MINECRAFT, "crafting_shapeless"),
        STONECUTTING = crate::id!(crate::ns::MINECRAFT, crate::recipe_kind::STONECUTTING),
        SMITHING_TRANSFORM = crate::id!(crate::ns::MINECRAFT, "smithing_transform"),
        /// A cooking recipe's time.
        COOKING_TIME = "cookingtime",
        /// A result's item: `id` since 1.20.5, `item` before.
        ID = crate::word::ID,
        ITEM = crate::kind::ITEM,
        TAG = crate::kind::TAG,
        /// What a recipe makes when it says nothing.
        AIR = crate::id!(crate::ns::MINECRAFT, "air"),
    }

    /// The game's worldgen data: `data/<ns>/worldgen/…`.
    worldgen {
        DIR = "worldgen",
        /// Its folders of features: what one is, and where it's placed.
        CONFIGURED_FEATURE = "configured_feature",
        PLACED_FEATURE = "placed_feature",
        /// The kinds of feature a `.ydef` makes.
        TREE = crate::id!(crate::ns::MINECRAFT, crate::kind::TREE),
        ORE = crate::id!(crate::ns::MINECRAFT, crate::kind::ORE),
        /// What blocks a feature places: one, or some by weight; one by rules.
        SIMPLE_STATE_PROVIDER = crate::id!(crate::ns::MINECRAFT, "simple_state_provider"),
        WEIGHTED_STATE_PROVIDER = crate::id!(crate::ns::MINECRAFT, "weighted_state_provider"),
        RULE_BASED_STATE_PROVIDER = crate::id!(crate::ns::MINECRAFT, "rule_based_state_provider"),
        /// Block predicates: not, in a tag.
        NOT = crate::id!(crate::ns::MINECRAFT, "not"),
        MATCHING_BLOCK_TAG = crate::id!(crate::ns::MINECRAFT, "matching_block_tag"),
        /// What a tree's trunk doesn't put dirt over.
        CANNOT_REPLACE_BELOW_TREE_TRUNK = crate::id!(crate::ns::MINECRAFT, "cannot_replace_below_tree_trunk"),
        /// A tree's size check.
        TWO_LAYERS_FEATURE_SIZE = crate::id!(crate::ns::MINECRAFT, "two_layers_feature_size"),
        /// What a tree grows on when it doesn't say.
        DIRT = crate::id!(crate::ns::MINECRAFT, "dirt"),
        /// A placer's id: `minecraft:<shape>_<part>_placer`.
        TRUNK = "trunk",
        FOLIAGE = "foliage",
        PLACER = "placer",
        /// What an ore replaces: blocks of a tag, or one block.
        TAG_MATCH = crate::id!(crate::ns::MINECRAFT, "tag_match"),
        BLOCK_MATCH = crate::id!(crate::ns::MINECRAFT, "block_match"),
        /// Heights an ore is spread over: evenly, or most in the middle.
        UNIFORM = crate::id!(crate::ns::MINECRAFT, "uniform"),
        TRAPEZOID = crate::id!(crate::ns::MINECRAFT, "trapezoid"),
        /// Where a placed feature goes: so many a chunk, spread in it, at
        /// heights, in the biomes that have it.
        COUNT = crate::id!(crate::ns::MINECRAFT, crate::word::COUNT),
        IN_SQUARE = crate::id!(crate::ns::MINECRAFT, "in_square"),
        HEIGHT_RANGE = crate::id!(crate::ns::MINECRAFT, "height_range"),
        BIOME = crate::id!(crate::ns::MINECRAFT, crate::kind::BIOME),
        /// The biomes an ore is in when it doesn't say.
        IS_OVERWORLD = crate::id!(crate::ns::MINECRAFT, "is_overworld"),
        /// The generation step ores are placed in.
        UNDERGROUND_ORES = "underground_ores",
    }

    /// The biome modifiers Forge and NeoForge add features to biomes with:
    /// `data/<ns>/<loader>/biome_modifier/…`.
    biome_modifier {
        DIR = "biome_modifier",
        FORGE_ADD_FEATURES = crate::id!(crate::loader::FORGE, "add_features"),
        NEOFORGE_ADD_FEATURES = crate::id!(crate::loader::NEOFORGE, "add_features"),
    }

    /// The tags table (yog-formats' `data/tags.txt`, copied for the hosts):
    /// what its lines start with, and a tag every host has.
    tag_table {
        TAG = crate::kind::TAG,
        YOG = crate::ns::YOG,
        BASE = "base",
        HAS = "has",
        EVERYWHERE = "*",
        LOADERS = "# loaders:",
    }

    /// The game's advancements.
    advancement {
        /// How one is shown: a task, a goal, a challenge.
        TASK = "task",
        GOAL = "goal",
        CHALLENGE = "challenge",
        /// What earns it: the mod's code, an item had, an item used up.
        IMPOSSIBLE = crate::id!(crate::ns::MINECRAFT, "impossible"),
        INVENTORY_CHANGED = crate::id!(crate::ns::MINECRAFT, "inventory_changed"),
        CONSUME_ITEM = crate::id!(crate::ns::MINECRAFT, "consume_item"),
    }

    /// The game's tool tiers' tags of what they can't mine; the weakest's.
    tool_tier {
        INCORRECT_FOR_WOODEN_TOOL = crate::id!(crate::ns::MINECRAFT, "incorrect_for_wooden_tool"),
    }
}
