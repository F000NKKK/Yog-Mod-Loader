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

    /// Yog's chunk generator in code (`yog:callback_generator`): the hosts
    /// write a dimension's JSON with it, and read it back.
    callback_generator {
        PATH = "callback_generator",
        TYPE = crate::id!(crate::ns::YOG, PATH),
        GENERATOR_TYPE_ID = "generator_type_id",
        MIN_Y = "min_y",
        HEIGHT = crate::word::HEIGHT,
        STRUCTURES = crate::dimension_field::STRUCTURES,
        FEATURES = crate::dimension_field::FEATURES,
        MOB_SPAWNING = crate::dimension_field::MOB_SPAWNING,
        BIOME_SOURCE = "biome_source",
    }

    /// The game's dimensions as a datapack writes them.
    game_dimension {
        DIMENSION_TYPE = crate::dimension_field::DIMENSION_TYPE,
        DIMENSION = crate::kind::DIMENSION,
        /// A dimension's biomes: all of one.
        FIXED_BIOMES = crate::id!(crate::ns::MINECRAFT, "fixed"),
        /// The game's own generator and its settings, for a dimension with no generator of Yog's.
        NOISE = crate::id!(crate::ns::MINECRAFT, "noise"),
        OVERWORLD_NOISE = crate::game::OVERWORLD,
        /// What burns forever in it.
        INFINIBURN = crate::concatcp!(crate::sep::TAG, crate::id!(crate::ns::MINECRAFT, "infiniburn_overworld")),
        /// Its look before 26.1 (`effects`).
        EFFECTS_OVERWORLD = crate::game::OVERWORLD,
        EFFECTS_END = crate::game::THE_END,
        EFFECTS_NETHER = crate::game::THE_NETHER,
        /// 26.x attributes: whether beds work and explode, whether respawn anchors work.
        BED_RULE = crate::id!(crate::ns::MINECRAFT, "gameplay/bed_rule"),
        ANCHORS_WORK = crate::id!(crate::ns::MINECRAFT, "gameplay/respawn_anchor_works"),
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
