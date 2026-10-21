//! `.ydef` data: renamed content, ores, advancements, trees, tags, dimensions.

use super::*;

#[test]
fn data_naming_renamed_content_gets_each_versions_name() {
    let model =
        r#"model: { parent: "cube_all", textures: { all: "@texture:minecraft:block/stone" } }"#;
    let block =
        format!(r#"{{ {model}, block: {{ drops: {{ item: "@item:minecraft:short_grass" }} }} }}"#);
    let c = compile(
        &project(&[("content/tuft.yblock", &block)]),
        "mymod",
        &yres::Refs::default(),
    )
    .unwrap();
    let loot = |p: &str| {
        c.outputs
            .iter()
            .find(|(q, _)| q == p)
            .map(|(_, b)| String::from_utf8_lossy(b).to_string())
    };
    assert!(
        loot("data/mymod/loot_table/blocks/tuft.json")
            .is_some_and(|l| l.contains("minecraft:short_grass")),
        "the datapack's name"
    );
    let old =
        loot("when/any/1.20.1/data/mymod/loot_table/blocks/tuft.json").expect("1.20.1's overlay");
    assert!(
        old.contains("minecraft:grass") && !old.contains("short_grass"),
        "{old}"
    );
    let crafter =
        format!(r#"{{ {model}, block: {{ drops: {{ item: "@item:minecraft:crafter" }} }} }}"#);
    let c = compile(
        &project(&[("content/c.yblock", &crafter)]),
        "mymod",
        &yres::Refs::default(),
    )
    .unwrap();
    assert!(
        c.warnings
            .iter()
            .any(|w| w.contains("minecraft:crafter, which Minecraft 1.20.1 hasn't got")),
        "{:?}",
        c.warnings
    );
}

#[test]
fn ores_become_features_in_biomes_on_every_loader() {
    let model =
        r#"model: { parent: "cube_all", textures: { all: "@texture:minecraft:block/stone" } }"#;
    let block = format!(r#"{{ {model} }}"#);
    let ore = r#"{ kind: "ore", id: "mythril_ore", targets: [{ replaces: "stone", block: "@block:mymod:mythril_ore" },
        { replaces: "@block:minecraft:basalt", block: "@block:mymod:mythril_ore" }], size: 6, per_chunk: 4,
        height: { min: -48, max: 16, shape: "triangle" } }"#;
    let c = compile(
        &project(&[
            ("content/mythril_ore.yblock", &block),
            ("content/mythril_ore.ydef", ore),
        ]),
        "mymod",
        &yres::Refs::default(),
    )
    .unwrap();
    let get = |p: &str| {
        c.outputs
            .iter()
            .find(|(q, _)| q == p)
            .map(|(_, b)| serde_json::from_slice::<Value>(b).unwrap())
            .unwrap_or_else(|| panic!("no {p}"))
    };
    let cf = get("data/mymod/worldgen/configured_feature/mythril_ore.json");
    assert_eq!(
        cf["config"]["targets"][0]["target"]["tag"],
        "minecraft:stone_ore_replaceables"
    );
    assert_eq!(
        cf["config"]["targets"][1]["target"]["block"],
        "minecraft:basalt"
    );
    let pf = get("data/mymod/worldgen/placed_feature/mythril_ore.json");
    assert_eq!(pf["placement"][2]["height"]["type"], "minecraft:trapezoid");
    assert_eq!(
        get("when/neoforge/1.21/data/mymod/neoforge/biome_modifier/mythril_ore.json")["type"],
        "neoforge:add_features"
    );
    assert_eq!(
        get("when/forge/any/data/mymod/forge/biome_modifier/mythril_ore.json")["biomes"],
        "#minecraft:is_overworld"
    );
    let src = c.source.unwrap().all();
    assert!(src.contains(r#"registry.add_biome_feature("mymod:mythril_ore", "minecraft:is_overworld", "underground_ores");"#), "{src}");
    let bad = r#"{ kind: "ore", id: "x", targets: [{ replaces: "granite", block: "@block:mymod:nope" }], size: 6, per_chunk: 4, height: { min: 0, max: 10 } }"#;
    let e = compile(
        &project(&[("content/x.ydef", bad)]),
        "mymod",
        &yres::Refs::default(),
    )
    .unwrap_err();
    assert!(
        e.contains("`granite`: one of stone, deepslate, netherrack, end_stone"),
        "{e}"
    );
    let missing = r#"{ kind: "ore", id: "x", targets: [{ replaces: "stone", block: "@block:mymod:nope" }], size: 6, per_chunk: 4, height: { min: 0, max: 10 } }"#;
    let e = compile(
        &project(&[("content/x.ydef", missing)]),
        "mymod",
        &yres::Refs::default(),
    )
    .unwrap_err();
    assert!(e.contains("no block mymod:nope"), "{e}");
}

#[test]
fn advancements_are_definitions_for_every_version() {
    let root = r#"{ kind: "advancement", id: "root", icon: "@item:minecraft:amethyst_shard", title: "Gems",
        background: "minecraft:textures/block/calcite.png", criteria: [{ has_item: "@tag:item:yog:gems/amethyst" }] }"#;
    let lore = r#"{ kind: "advancement", id: "lore/one", parent: "@advancement:mymod:root", icon: "@item:minecraft:paper",
        title: "@lang:advancement.mymod.lore.one", hidden: true, criteria: [{ granted: true }] }"#;
    let c = compile(
        &project(&[("content/root.ydef", root), ("content/lore.ydef", lore)]),
        "mymod",
        &yres::Refs::default(),
    )
    .unwrap();
    let get = |p: &str| {
        c.outputs
            .iter()
            .find(|(q, _)| q == p)
            .map(|(_, b)| serde_json::from_slice::<Value>(b).unwrap())
            .unwrap_or_else(|| panic!("no {p}"))
    };
    let new = get("data/mymod/advancement/root.json");
    assert_eq!(new["display"]["icon"]["id"], "minecraft:amethyst_shard");
    assert_eq!(
        new["criteria"]["c0"]["conditions"]["items"][0]["items"],
        "#yog:gems/amethyst"
    );
    assert_eq!(
        new["display"]["title"]["translate"],
        "advancement.mymod.root"
    );
    let old = get("when/any/1.20.1/data/mymod/advancement/root.json");
    assert_eq!(old["display"]["icon"]["item"], "minecraft:amethyst_shard");
    assert_eq!(
        old["criteria"]["c0"]["conditions"]["items"][0]["tag"],
        "yog:gems/amethyst"
    );
    let one = get("data/mymod/advancement/lore/one.json");
    assert_eq!(
        (
            one["parent"].as_str(),
            one["criteria"]["c0"]["trigger"].as_str()
        ),
        (Some("mymod:root"), Some("minecraft:impossible"))
    );
    assert_eq!(
        c.lang[&("mymod".to_string(), yres::NEUTRAL_LOCALE.to_string())]["advancement.mymod.root"],
        "Gems"
    );
    // The code names them by constant.
    let src = &c.source.as_ref().unwrap().all();
    assert!(
        src.contains("pub const LORE_ONE: &'static str = \"mymod:lore/one\";"),
        "{src}"
    );
    let bad = r#"{ kind: "advancement", id: "x", icon: "@item:minecraft:paper", title: "X", criteria: [{ granted: true, used_item: "@item:minecraft:paper" }] }"#;
    assert!(
        compile(
            &project(&[("content/x.ydef", bad)]),
            "mymod",
            &yres::Refs::default()
        )
        .unwrap_err()
        .contains("one of granted")
    );
}

#[test]
fn trees_are_definitions() {
    let model =
        r#"model: { parent: "cube_all", textures: { all: "@texture:minecraft:block/stone" } }"#;
    let block = format!(r#"{{ {model} }}"#);
    let tree = r#"{ kind: "tree", id: "glass_tree", min_clipped_height: 6,
        trunk: { blocks: [{ block: "@block:mymod:log", weight: 8 }, { block: "@block:minecraft:oak_log" }], shape: "fancy", base_height: 5, random_a: 5, random_b: 3 },
        leaves: { blocks: [{ block: "@block:mymod:leaves" }], shape: "fancy", radius: 1, offset: 5, height: 5 } }"#;
    let c = compile(
        &project(&[
            ("content/log.yblock", &block),
            ("content/leaves.yblock", &block),
            ("content/glass_tree.ydef", tree),
        ]),
        "mymod",
        &yres::Refs::default(),
    )
    .unwrap();
    let get = |p: &str| {
        c.outputs
            .iter()
            .find(|(q, _)| q == p)
            .map(|(_, b)| serde_json::from_slice::<Value>(b).unwrap())
            .unwrap_or_else(|| panic!("no {p}"))
    };
    let f = &get("data/mymod/worldgen/configured_feature/glass_tree.json")["config"];
    assert_eq!(f["trunk_placer"]["type"], "minecraft:fancy_trunk_placer");
    assert_eq!(f["trunk_provider"]["entries"][0]["weight"], 8);
    assert_eq!(f["foliage_provider"]["state"]["Name"], "mymod:leaves");
    assert_eq!(
        (
            f["foliage_placer"]["height"].as_u64(),
            f["minimum_size"]["min_clipped_height"].as_u64()
        ),
        (Some(5), Some(6))
    );
    let bad = r#"{ kind: "tree", id: "t", trunk: { blocks: [{ block: "@block:mymod:nope" }], shape: "straight", base_height: 4 },
        leaves: { blocks: [{ block: "@block:minecraft:oak_leaves" }], shape: "blob", radius: 2 } }"#;
    let e = compile(
        &project(&[("content/t.ydef", bad)]),
        "mymod",
        &yres::Refs::default(),
    )
    .unwrap_err();
    assert!(e.contains("no block mymod:nope"), "{e}");
}

#[test]
fn tags_and_damage_types_are_definitions() {
    let tag = r#"{ kind: "tag", of: "entity", id: "sticky_teleporters", values: ["@entity:minecraft:pig", "@tag:entity:c:bosses"],
        optional: ["@entity:create:seat"] }"#;
    let logs =
        r#"{ kind: "tag", of: "block", id: "minecraft:logs", values: ["@block:minecraft:stone"] }"#;
    let dmg = r#"{ kind: "damage_type", id: "overcast", scaling: "never", tags: ["minecraft:bypasses_armor", "bypasses_effects"] }"#;
    let c = compile(
        &project(&[
            ("content/a.ydef", tag),
            ("content/b.ydef", logs),
            ("content/c.ydef", dmg),
        ]),
        "mymod",
        &yres::Refs::default(),
    )
    .unwrap();
    let sticky = &c.tags["data/mymod/tags/entity_type/sticky_teleporters.json"];
    assert_eq!(
        sticky
            .iter()
            .map(|(k, v)| (k.as_str(), *v))
            .collect::<Vec<_>>(),
        vec![
            ("#c:bosses", false),
            ("create:seat", false),
            ("minecraft:pig", true)
        ],
        "c:bosses isn't on every loader: where it isn't, the tag still loads"
    );
    assert!(c.tags["data/minecraft/tags/block/logs.json"].contains_key("minecraft:stone"));
    let get = |p: &str| {
        c.outputs
            .iter()
            .find(|(q, _)| q == p)
            .map(|(_, b)| serde_json::from_slice::<Value>(b).unwrap())
            .unwrap_or_else(|| panic!("no {p}"))
    };
    let d = get("data/mymod/damage_type/overcast.json");
    assert_eq!(
        (d["message_id"].as_str(), d["scaling"].as_str()),
        (Some("mymod.overcast"), Some("never"))
    );
    for t in [
        "data/minecraft/tags/damage_type/bypasses_armor.json",
        "data/minecraft/tags/damage_type/bypasses_effects.json",
    ] {
        assert!(c.tags[t].contains_key("mymod:overcast"), "{t}");
    }
    let bad = r#"{ kind: "tag", of: "entity", id: "x", values: ["@entity:minecraft:nope", "@block:minecraft:stone"] }"#;
    let e = compile(
        &project(&[("content/x.ydef", bad)]),
        "mymod",
        &yres::Refs::default(),
    )
    .unwrap_err();
    assert!(
        e.contains("nope") && e.contains("`@block:minecraft:stone` in a tag of entity"),
        "{e}"
    );
}

#[test]
fn a_dimension_and_its_terrain_go_to_the_runtime_as_data() {
    let dim = r#"{ kind: "dimension", id: "flatlands", dimension_type: { has_sky_light: true }, biome: "@biome:minecraft:desert",
        terrain: { seed: 3, base_height: 70, noise: [{ scale: 80, amplitude: 6 }], fill: "@block:minecraft:sandstone",
            layers: [{ block: "@block:minecraft:sand", depth: 3 }] } }"#;
    let c = compile(
        &project(&[("content/flatlands.ydef", dim)]),
        "mymod",
        &yres::Refs::default(),
    )
    .unwrap();
    let (path, data) = c
        .data_files
        .iter()
        .find(|(p, _)| p.starts_with("dimensions/"))
        .expect("the dimension's data");
    assert_eq!(path, "dimensions/mymod__flatlands.ydata");
    let v = yog_data::binary::decode(data).unwrap();
    assert_eq!(
        v.get("biome"),
        Some(&yog_data::Value::from("minecraft:desert")),
        "a plain id: the hosts read it"
    );
    assert_eq!(
        v.get("generator_type"),
        Some(&yog_data::Value::from("mymod:flatlands"))
    );
    let t = yog_data::read_value::<yog_worldgen::Terrain>(v.get("terrain").unwrap()).unwrap();
    assert_eq!(
        (t.base_height, t.fill.as_str()),
        (70, "minecraft:sandstone")
    );
    let src = c.source.unwrap().all();
    assert!(
        src.contains(r#"registry.register_dimension_data("mymod:flatlands", include_bytes!("#),
        "{src}"
    );
    let bad = r#"{ kind: "dimension", id: "x", terrain: { fill: "@block:minecraft:nope" } }"#;
    let e = compile(
        &project(&[("content/x.ydef", bad)]),
        "mymod",
        &yres::Refs::default(),
    )
    .unwrap_err();
    assert!(e.contains("no block minecraft:nope"), "{e}");
}

#[test]
fn content_for_other_mods_is_registered_only_with_them() {
    let model =
        r#"model: { parent: "generated", textures: { layer0: "@texture:minecraft:item/stick" } }"#;
    let item = format!(r#"{{ requires: ["pehkui", "create"], {model} }}"#);
    let recipe = r#"{ requires: ["pehkui"], type: "shapeless", ingredients: ["@item:minecraft:stick"], result: "@item:minecraft:stick" }"#;
    let c = compile(
        &project(&[
            ("content/shrink.yitem", &item),
            ("content/r.yrecipe", recipe),
        ]),
        "mymod",
        &yres::Refs::default(),
    )
    .unwrap();
    let src = c.source.unwrap().all();
    assert!(src.contains(r#"if ::yog_api::mod_loaded("pehkui") && ::yog_api::mod_loaded("create") { registry.register_item(Items::shrink()); }"#), "{src}");
    assert!(
        src.contains(r#"if ::yog_api::mod_loaded("pehkui") { registry.add_recipe("#),
        "{src}"
    );
}
