//! Content registration — declare custom items and blocks from Rust.
//!
//! Definitions are collected at registration time and handed to the host, which
//! registers real `Item`/`Block` objects before the game's registries freeze,
//! puts them in per-namespace creative tabs, and applies their properties.
//! (Textures, models and recipes are assets/data shipped in the `.yog` and
//! served to the game.)
//!
//! ```
//! # use yog_registry::{ItemDef, BlockDef, FoodDef};
//! ItemDef::new("mymod:ruby").name("Ruby").tooltip("A shiny gem.").max_stack(16);
//! ItemDef::new("mymod:pie").food(FoodDef::new(4, 0.3));
//! BlockDef::new("mymod:lamp").light_level(15).sound("stone");
//! ```

pub mod atlases;
pub mod effects;
pub mod item_looks;
pub mod loot;
pub mod particles;
pub mod recipe_kinds;
pub mod tool_levels;

/// What a `.yog` registers when the game starts — content the game can't
/// add or change while it runs (blocks, items, entity types, sound groups,
/// materials, dimensions): a line each, `<kind> <id> <hash>`, the hash of
/// what can't change. A hot reload compares the new `.yog`'s with what's
/// registered ([`registration_changes`]).
pub const REGISTRATIONS_FILE: &str = "registrations.txt";

/// How `new` (a [`REGISTRATIONS_FILE`]) differs from `registered`: what it
/// adds, removes and changes — `<kind> <id>` each.
pub fn registration_changes(
    registered: &str,
    new: &str,
) -> (Vec<String>, Vec<String>, Vec<String>) {
    fn read(text: &str) -> std::collections::BTreeMap<String, String> {
        text.lines()
            .filter_map(|l| {
                let (key, hash) = l.trim().rsplit_once(' ')?;
                Some((key.to_string(), hash.to_string()))
            })
            .collect()
    }
    let (old, new) = (read(registered), read(new));
    let added = new
        .keys()
        .filter(|k| !old.contains_key(*k))
        .cloned()
        .collect();
    let removed = old
        .keys()
        .filter(|k| !new.contains_key(*k))
        .cloned()
        .collect();
    let changed = new
        .iter()
        .filter(|(k, h)| old.get(*k).is_some_and(|o| o != *h))
        .map(|(k, _)| k.clone())
        .collect();
    (added, removed, changed)
}
pub use atlases::Atlas;
pub use effects::{
    AttributeDef, Brew, EffectCategory, EffectDef, EffectModifier, ModifierOperation, PotionDef,
    PotionEffect,
};
pub use item_looks::{ItemLooks, ItemTint, LookModels};
pub use loot::{Loot, LootAdd, LootModifier, LootScale, LootStack};
pub use particles::{Blend, Frames, Lifetime, Particle, ParticleDef, ParticleRender};
pub use tool_levels::{LevelTag, ToolLadder, ToolLevel};

mod blocks;
mod entities;
mod equipment;
mod items;
mod recipes;
mod sounds;
mod worldgen;
pub use blocks::{BlockDef, BlockProperty, Facing, Placement};
pub use entities::{EntityDef, EntityKind};
pub use equipment::{
    ArmorDefense, ArmorMaterial, ArmorPiece, ArmorSlot, DIGGERS, GameTool, REFERENCE_KIND,
    REFERENCE_MATERIAL, TOOL_KINDS, ToolItem, ToolMaterial,
};
pub use items::{FoodDef, Hangs, ItemDef};
pub use recipes::{COOK_EXPERIENCE, Cooking, Recipe, RecipeKind, StartupGrant};
pub use sounds::SoundGroup;
pub use worldgen::BiomeFeature;

#[cfg(test)]
mod registration_tests {
    #[test]
    fn a_hot_reload_sees_what_registration_changed() {
        let old = "block mymod:a 11\nblock mymod:b 22\nitem mymod:c 33\n";
        let new = "block mymod:a 11\nblock mymod:b 99\nitem mymod:d 44\n";
        let (added, removed, changed) = super::registration_changes(old, new);
        assert_eq!(added, ["item mymod:d"]);
        assert_eq!(removed, ["item mymod:c"]);
        assert_eq!(changed, ["block mymod:b"]);
        assert_eq!(
            super::registration_changes(old, old),
            (vec![], vec![], vec![])
        );
    }
}

#[cfg(test)]
mod material_tests {
    use super::*;

    #[test]
    fn the_games_materials_are_built_in() {
        assert_eq!(ToolMaterial::built_ins().len(), 6);
        let iron = ToolMaterial::built_in("iron").unwrap();
        assert_eq!(
            (iron.durability, iron.level_index, iron.incorrect.as_str()),
            (250, 2, "minecraft:incorrect_for_iron_tool")
        );
        assert_eq!(
            ArmorMaterial::built_in("minecraft:netherite")
                .unwrap()
                .knockback_resistance,
            0.1
        );
        assert_eq!(ArmorMaterial::built_in("diamond").unwrap().defense.chest, 8);
        let mythril = ToolMaterial::like("mymod:mythril", "diamond").unwrap();
        assert_eq!(
            (mythril.id.as_str(), mythril.durability),
            ("mymod:mythril", 1561)
        );
    }
}

#[cfg(test)]
mod sound_tests {
    use super::*;

    #[test]
    fn the_games_sound_groups_are_built_in() {
        assert!(SoundGroup::built_ins().len() > 100);
        let stone = SoundGroup::built_in("stone").unwrap();
        assert_eq!(
            (stone.break_sound.as_str(), stone.volume),
            ("minecraft:block.stone.break", 1.0)
        );
        assert_eq!(SoundGroup::built_in("minecraft:metal").unwrap().pitch, 1.5);
        assert!(SoundGroup::built_in("mymod:crystal").is_none());
    }

    #[test]
    fn a_mods_group_crosses_as_yog_data() {
        use yog_data::Schema;
        let g = SoundGroup::new("mymod:crystal").pitch(1.2);
        assert_eq!(g.fall, "mymod:crystal.fall");
        let back = yog_data::from_value::<SoundGroup>(&g.write())
            .unwrap()
            .value;
        assert_eq!(back, g);
        assert_eq!(
            g.write().get("break").and_then(|v| v.as_str()),
            Some("mymod:crystal.break")
        );
    }
}

#[cfg(test)]
impl Recipe {
    fn to_json(&self) -> String {
        yog_data::json::to_json(&self.to_data()).to_string()
    }
}

#[cfg(test)]
mod recipe_tests {
    use super::*;

    #[test]
    fn writes_the_yog_format() {
        let js = |s: &str| serde_json::from_str::<serde_json::Value>(s).unwrap();
        let shaped = Recipe::shaped("m:a", "m:b", 2)
            .row("D ")
            .key('D', "#c:dusts")
            .key_any('E', ["m:x", "m:y"])
            .group("g");
        assert_eq!(
            js(&shaped.to_json()),
            js(
                r##"{"type":"shaped","pattern":["D "],"key":{"D":"#c:dusts","E":["m:x","m:y"]},"result":"m:b","count":2,"group":"g"}"##
            )
        );
        assert_eq!(
            shaped.to_data().as_map().unwrap().keys().next(),
            Some("type"),
            "the type first"
        );
        let blast = Recipe::cooking("m:a", Cooking::Blasting, "#c:ores", "m:b", 1).experience(0.7);
        assert_eq!(
            js(&blast.to_json()),
            js(
                r##"{"type":"blasting","ingredient":"#c:ores","experience":0.7,"cook_time":100,"result":"m:b"}"##
            )
        );
        let smith = Recipe::smithing("m:a", "m:t", "m:base", "m:add", "m:out");
        assert_eq!(js(&smith.to_json())["template"], "m:t");
    }
}

#[cfg(test)]
mod json_tests {
    use super::*;

    /// Every `to_json` makes valid JSON, whatever the strings hold.
    #[test]
    fn json_survives_any_string() {
        let odd = "m:a\"b\\c\nd\te\u{1}";
        let parse = |j: String| {
            serde_json::from_str::<serde_json::Value>(&j).unwrap_or_else(|e| panic!("{e}: {j}"))
        };
        for r in [
            Recipe::shaped(odd, odd, 1)
                .row("A")
                .key('A', odd)
                .group(odd)
                .category(odd),
            Recipe::shapeless(odd, odd, 3)
                .ingredient(odd)
                .ingredient_any([odd, "m:x"]),
            Recipe::cooking(odd, Cooking::Smoking, odd, odd, 1).cook_time(5),
            Recipe::stonecutting(odd, odd, odd, 2),
            Recipe::smithing(odd, odd, odd, odd, odd),
        ] {
            let v = parse(r.to_json());
            assert_eq!(v["result"], odd, "the string comes back as it was");
        }
    }

    #[test]
    fn recipes_and_grants_name_their_parts() {
        let r = Recipe::cooking(
            "mymod:a",
            Cooking::Campfire,
            "minecraft:beef",
            "minecraft:cooked_beef",
            1,
        );
        let v: serde_json::Value = serde_json::from_str(&r.to_json()).unwrap();
        assert_eq!(v["type"], "campfire_cooking");
        assert_eq!(v["cook_time"], 600, "the method's default time");
        assert_eq!(r.ns_name(), ("mymod", "a"));
        assert_eq!(Recipe::shaped("plain", "x", 1).ns_name().1, "plain");
    }
}

/// The tables yog-registry ships read: none falls back to empty in the game.
#[cfg(test)]
mod built_in_tests {
    use super::*;

    #[test]
    fn every_built_in_table_reads() {
        let reads = |name: &str, ok: bool| assert!(ok, "{name} won't read");
        reads(
            "tool materials",
            yog_data::from_text::<Vec<ToolMaterial>>(include_str!(
                "../assets/tool_materials.ydata"
            ))
            .is_ok(),
        );
        reads(
            "armor materials",
            yog_data::from_text::<Vec<ArmorMaterial>>(include_str!(
                "../assets/armor_materials.ydata"
            ))
            .is_ok(),
        );
        reads(
            "sound groups",
            yog_data::from_text::<Vec<SoundGroup>>(include_str!("../assets/sound_groups.ydata"))
                .is_ok(),
        );
        reads(
            "item looks",
            yog_data::from_text::<item_looks::ItemLooks>(include_str!(
                "../assets/item_looks.ydata"
            ))
            .is_ok(),
        );
        reads(
            "tool levels",
            yog_data::from_text::<tool_levels::ToolLadder>(include_str!(
                "../assets/tool_levels.ydata"
            ))
            .is_ok(),
        );
        reads(
            "atlases",
            yog_data::from_text::<Vec<atlases::Atlas>>(include_str!("../assets/atlases.ydata"))
                .is_ok(),
        );
        reads(
            "recipe kinds",
            yog_data::from_text::<Vec<recipe_kinds::RecipeKindDef>>(include_str!(
                "../data/recipe_kinds.ydata"
            ))
            .is_ok(),
        );
    }
}
