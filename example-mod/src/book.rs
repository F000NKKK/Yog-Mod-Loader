//! Example Mod guide book — documents all items, blocks, commands and recipes.

use yog_api::{
    Book, BookCategory, BookEntry,
    crafting_page, smelting_page, spotlight_page, text_page,
    ItemDef,
};

pub fn guide_book() -> Book {
    Book::new("yog:example_guide", "Example Mod Guide")
        .nameplate("aa0000")
        .landing_text("Welcome to the Example Mod! This book documents all the items, blocks, commands and crafting recipes added by this mod.")
        .author("Yog Team")
        .creative_tab("yog")
        .add_category(BookCategory {
            id: "items".into(), name: "Items".into(),
            description: Some("Custom items added by the mod.".into()),
            icon: Some("yog:item/ruby".into()), icon_svg: None, sortnum: 0,
        })
        .add_category(BookCategory {
            id: "blocks".into(), name: "Blocks".into(),
            description: Some("Custom blocks and their properties.".into()),
            icon: Some("yog:item/ruby_block".into()), icon_svg: None, sortnum: 1,
        })
        .add_category(BookCategory {
            id: "commands".into(), name: "Commands".into(),
            description: Some("Chat commands you can use.".into()),
            icon: Some("minecraft:item/command_block".into()), icon_svg: None, sortnum: 2,
        })
        .add_category(BookCategory {
            id: "crafting".into(), name: "Crafting".into(),
            description: Some("Recipes added by the mod.".into()),
            icon: Some("minecraft:item/crafting_table".into()), icon_svg: None, sortnum: 3,
        })
        // ── Items ──────────────────────────────────────────────────────────
        .add_entry(BookEntry {
            id: "ruby".into(), name: "Ruby".into(), category: "items".into(),
            icon: Some("yog:ruby".into()),
            pages: vec![
                spotlight_page(ItemDef::new("yog:ruby")
                    .name("Ruby").tooltip("A shiny gem, forged in Rust.").max_stack(16)),
                text_page("Rubies are the core resource. Used to craft Ruby Blocks.\nMax stack: 16. Obtain via /ruby."),
            ],
            ..Default::default()
        })
        .add_entry(BookEntry {
            id: "ruby_shard".into(), name: "Ruby Shard".into(), category: "items".into(),
            icon: Some("yog:ruby_shard".into()),
            pages: vec![
                spotlight_page(ItemDef::new("yog:ruby_shard")
                    .name("Ruby Shard").tooltip("Technically edible. Technically.")),
                text_page("Edible ruby shard. Restores 2 drumsticks (4 hunger), 0.3 saturation."),
            ],
            ..Default::default()
        })
        .add_entry(BookEntry {
            id: "ember_coal".into(), name: "Ember Coal".into(), category: "items".into(),
            icon: Some("yog:ember_coal".into()),
            pages: vec![
                spotlight_page(ItemDef::new("yog:ember_coal")
                    .name("Ember Coal").tooltip("Burns twice as long as regular coal.")),
                text_page("Fuel: 400 ticks (2 items smelted). Obtained by smelting coal."),
            ],
            ..Default::default()
        })
        // ── Blocks ─────────────────────────────────────────────────────────
        .add_entry(BookEntry {
            id: "ruby_block".into(), name: "Block of Ruby".into(), category: "blocks".into(),
            icon: Some("yog:ruby_block".into()),
            pages: vec![
                text_page("Storage block. Requires pickaxe.\nHardness: 3.0 | Resist: 6.0 | Sound: Metal"),
                crafting_page("yog:ruby_block_from_rubies"),
            ],
            ..Default::default()
        })
        .add_entry(BookEntry {
            id: "ember_block".into(), name: "Ember Block".into(), category: "blocks".into(),
            icon: Some("yog:ember_block".into()),
            pages: vec![
                text_page("Glowing block — light level 12.\nHardness: 0.5 | Resist: 0.5 | Sound: Stone"),
            ],
            ..Default::default()
        })
        // ── Commands ───────────────────────────────────────────────────────
        .add_entry(BookEntry {
            id: "cmd_yog".into(), name: "/yog".into(), category: "commands".into(),
            icon: Some("minecraft:command_block".into()),
            pages: vec![
                text_page("Usage: /yog [args]\n\nEchoes a greeting. Example: /yog hello → \"Yog here! You said: 'hello'\""),
            ],
            ..Default::default()
        })
        .add_entry(BookEntry {
            id: "cmd_ruby".into(), name: "/ruby".into(), category: "commands".into(),
            icon: Some("yog:ruby".into()),
            pages: vec![
                text_page("Usage: /ruby\n\nGives you one free Ruby in your inventory."),
            ],
            ..Default::default()
        })
        .add_entry(BookEntry {
            id: "cmd_loot".into(), name: "/loot".into(), category: "commands".into(),
            icon: Some("minecraft:chest".into()),
            pages: vec![
                text_page("Usage: /loot\n\nDrops zombie loot at (0, 64, 0) overworld. Demonstrates loot API."),
            ],
            ..Default::default()
        })
        // ── Recipes ────────────────────────────────────────────────────────
        .add_entry(BookEntry {
            id: "recipe_ruby_block".into(), name: "Ruby Block (craft)".into(), category: "crafting".into(),
            icon: Some("yog:ruby_block".into()),
            pages: vec![
                crafting_page("yog:ruby_block_from_rubies"),
                text_page("4 Rubies in a 2×2 → 1 Ruby Block. Works in inventory grid."),
            ],
            ..Default::default()
        })
        .add_entry(BookEntry {
            id: "recipe_ruby_decraft".into(), name: "Rubies from Block".into(), category: "crafting".into(),
            icon: Some("yog:ruby".into()),
            pages: vec![
                text_page("1 Ruby Block → 4 Rubies. Shapeless — position doesn't matter."),
            ],
            ..Default::default()
        })
        .add_entry(BookEntry {
            id: "recipe_ember_coal".into(), name: "Ember Coal (smelt)".into(), category: "crafting".into(),
            icon: Some("yog:ember_coal".into()),
            pages: vec![
                smelting_page("yog:ember_coal_smelting"),
                text_page("Smelt coal → Ember Coal. +0.5 XP. Burns 2× longer!"),
            ],
            ..Default::default()
        })
}