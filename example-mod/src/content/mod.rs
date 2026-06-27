//! Custom items and blocks declared by this mod.

use yog_api::{BlockDef, FoodDef, FurnaceRecipe, ItemDef, Registry, ShapedRecipe, ShapelessRecipe, StartupGrant};

pub fn register(registry: &mut Registry) {
    registry.register_item(
        ItemDef::new("yog:ruby")
            .name("Ruby")
            .tooltip("A shiny gem, forged in Rust.")
            .max_stack(16),
    );

    // Edible ruby shard — restores 4 hunger, 0.3 saturation.
    registry.register_item(
        ItemDef::new("yog:ruby_shard")
            .name("Ruby Shard")
            .tooltip("Technically edible. Technically.")
            .food(FoodDef::new(4, 0.3)),
    );

    // Ember coal — burns for 400 ticks (2 items smelted).
    registry.register_item(
        ItemDef::new("yog:ember_coal")
            .name("Ember Coal")
            .tooltip("Burns twice as long as regular coal.")
            .fuel(400),
    );

    registry.register_block(
        BlockDef::new("yog:ruby_block")
            .name("Block of Ruby")
            .strength(3.0, 6.0)
            .sound("metal")
            .requires_tool()
            // Custom hitbox: a smaller centred box.
            .shape(3.0, 0.0, 3.0, 13.0, 13.0, 13.0),
    );

    // Glowing ember block — emits light like a torch.
    registry.register_block(
        BlockDef::new("yog:ember_block")
            .name("Ember Block")
            .strength(0.5, 0.5)
            .light_level(12)
            .sound("stone"),
    );

    // ── Recipes ──────────────────────────────────────────────────────────────

    // 4 rubies in a 2×2 square → 1 ruby block.
    registry.add_shaped_recipe(
        ShapedRecipe::new("yog:ruby_block_from_rubies", "yog:ruby_block", 1)
            .row("RR")
            .row("RR")
            .key('R', "yog:ruby"),
    );

    // Ruby block → 4 rubies (shapeless).
    registry.add_shapeless_recipe(
        ShapelessRecipe::new("yog:rubies_from_block", "yog:ruby", 4)
            .ingredient("yog:ruby_block"),
    );

    // Smelt ember_coal from regular coal (just a demo).
    registry.add_furnace_recipe(
        FurnaceRecipe::new("yog:ember_coal_smelting", "minecraft:coal", "yog:ember_coal", 1)
            .experience(0.5),
    );

    // ── Guide book ─────────────────────────────────────────────────────────────

    // Guide book item for the Example Mod.
    registry.register_item(
        ItemDef::new("yog:example_guide")
            .name("Example Mod Guide")
            .tooltip("Everything you need to know."),
    );

    // Give the guide book to every new player on first join.
    registry.register_startup_grant(
        StartupGrant::new("yog:example_guide_grant")
            .item("yog:example_guide"),
    );
}
