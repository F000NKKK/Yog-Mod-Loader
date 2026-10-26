//! `.yblock`s: models, states, shapes, tools, drops, tags.

use super::*;

#[test]
fn compiles_a_cube_block() {
    let root = project(&[
        (
            "resources/block/ruby_ore.ytexture",
            r#"{ image: "@Images.block.ruby_ore" }"#,
        ),
        (
            "content/ruby.yitem",
            r#"{ model: { parent: "generated", textures: { layer0: "@texture:minecraft:item/emerald" } } }"#,
        ),
        (
            "content/blocks/ruby_ore.yblock",
            r#"{
  "name": "Ruby Ore",
  "block": { "hardness": 3, "resistance": 3, "sound": "stone", "tool": "pickaxe", "tool_level": "iron", "requires_tool": true,
         "drops": { "item": "@item:mymod:ruby", "count": [1, 3] } },
  "model": { "parent": "cube_all", "textures": { "all": "@texture:mymod:block/ruby_ore" } }
}"#,
        ),
    ]);
    let refs = pictures(&root, &["block.ruby_ore"]);
    let c = compile(&root, "mymod", &refs).unwrap();
    assert_eq!(c.blocks, 1);
    assert_eq!(
        output(&c, "assets/mymod/models/block/ruby_ore.json"),
        json!({ "parent": "minecraft:block/cube_all", "textures": { "all": "mymod:block/ruby_ore" } })
    );
    assert_eq!(
        output(&c, "assets/mymod/blockstates/ruby_ore.json"),
        json!({ "variants": { "": { "model": "mymod:block/ruby_ore" } } })
    );
    assert_eq!(
        output(&c, "assets/mymod/models/item/ruby_ore.json"),
        json!({ "parent": "mymod:block/ruby_ore" })
    );
    assert_eq!(
        output(&c, "assets/mymod/items/ruby_ore.json")["model"]["model"],
        json!("mymod:item/ruby_ore")
    );
    let loot = output(&c, "data/mymod/loot_table/blocks/ruby_ore.json");
    assert_eq!(loot["pools"][0]["entries"][0]["name"], json!("mymod:ruby"));
    assert_eq!(
        loot["pools"][0]["entries"][0]["functions"][0]["count"]["max"],
        json!(3)
    );
    assert!(
        c.outputs
            .iter()
            .any(|(p, _)| p == "assets/mymod/textures/block/ruby_ore.png")
    );
    let src = &c.source.as_ref().unwrap().all();
    assert!(
        src.contains("pub const RUBY_ORE: &'static str = \"mymod:ruby_ore\";"),
        "{src}"
    );
    assert!(src.contains(".strength(3.0, 3.0)"), "{src}");
    assert!(src.contains(".requires_tool()"), "{src}");
    assert!(
        src.contains("registry.register_block(Blocks::ruby_ore());"),
        "{src}"
    );
    // The mod's Yog Script names it the same way.
    let ys = c.script.as_deref().unwrap_or_default();
    assert!(ys.contains("public static class Blocks {"), "{ys}");
    assert!(
        ys.contains("    public const string RubyOre = \"mymod:ruby_ore\";"),
        "{ys}"
    );

    let mut outputs = Vec::new();
    merge_outputs(&root, &mut outputs, c).unwrap();
    let find = |p: &str| -> Value {
        serde_json::from_slice(&outputs.iter().find(|(q, _)| q == p).unwrap().1).unwrap()
    };
    assert_eq!(
        find("assets/mymod/lang/en_us.json")["block.mymod.ruby_ore"],
        json!("Ruby Ore")
    );
    assert_eq!(
        find("data/minecraft/tags/block/mineable/pickaxe.json")["values"],
        json!(["mymod:ruby_ore"])
    );
    assert_eq!(
        find("data/minecraft/tags/block/needs_iron_tool.json")["values"],
        json!(["mymod:ruby_ore"])
    );
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn raw_parent_with_elements_gets_a_particle() {
    let root = project(&[(
        "content/cable.yblock",
        r##"{ "model": { "parent": "minecraft:block/block", "textures": { "all": "@texture:minecraft:block/stone" },
             "elements": [ { "from": [5, 5, 5], "to": [11, 11, 11], "faces": { "up": { "texture": "#all" } } } ] } }"##,
    )]);
    let c = compile(&root, "mymod", &yres::Refs::default()).unwrap();
    let m = output(&c, "assets/mymod/models/block/cable.json");
    assert_eq!(m["textures"]["particle"], json!("minecraft:block/stone"));
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn a_facing_block_turns_to_its_placer() {
    let root = project(&[(
        "content/screen.yblock",
        r##"{ "model": { "parent": "cube_all", "textures": { "all": "@texture:minecraft:block/stone" } },
             "states": { "facing": "horizontal" } }"##,
    )]);
    let c = compile(&root, "mymod", &yres::Refs::default()).unwrap();
    let bs = output(&c, "assets/mymod/blockstates/screen.json");
    assert_eq!(
        bs["variants"]["facing=north"],
        json!({ "model": "mymod:block/screen" })
    );
    assert_eq!(bs["variants"]["facing=west"]["y"], json!(270));
    assert!(
        c.source
            .unwrap()
            .all()
            .contains(".facing(::yog_api::Facing::Horizontal, ::yog_api::Placement::Toward)")
    );
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn a_block_takes_its_materials_textures_and_layer() {
    let root = project(&[
        (
            "resources/block/glass.ymaterial",
            r#"{ textures: { all: "@texture:minecraft:block/glass", particle: "@texture:minecraft:block/sand" }, render_layer: "translucent" }"#,
        ),
        (
            "content/pane.yblock",
            r#"{ "model": { "parent": "cube_all", "material": "@material:mymod:block/glass", "textures": { "particle": "@texture:minecraft:block/stone" } } }"#,
        ),
    ]);
    let c = compile(&root, "mymod", &yres::Refs::default()).unwrap();
    let m = output(&c, "assets/mymod/models/block/pane.json");
    assert_eq!(m["textures"]["all"], json!("minecraft:block/glass"));
    assert_eq!(
        m["textures"]["particle"],
        json!("minecraft:block/stone"),
        "the model's own over the material's"
    );
    assert_eq!(m["render_type"], json!("minecraft:translucent"));
    let missing = project(&[(
        "content/x.yblock",
        r#"{ "model": { "parent": "cube_all", "material": "@material:mymod:block/none" } }"#,
    )]);
    assert!(
        compile(&missing, "mymod", &yres::Refs::default())
            .unwrap_err()
            .contains("material `mymod:block/none`")
    );
    let _ = std::fs::remove_dir_all(root);
    let _ = std::fs::remove_dir_all(missing);
}

#[test]
fn compiles_elements_shape_and_connect() {
    let root = project(&[(
        "content/pipe.yblock",
        r##"{
  "block": { "render_layer": "cutout" },
  "model": {
"textures": { "0": "@texture:minecraft:block/stone" },
"elements": [ { "from": [6, 6, 6], "to": [10, 10, 10],
                "rotation": { "axis": "y", "angle": 22.5 },
                "faces": { "north": { "texture": "#0", "uv": [0, 0, 4, 4], "cullface": "north" } } } ]
  },
  "states": { "connect": { "arm": { "textures": { "0": "@texture:minecraft:block/stone" },
 "elements": [ { "from": [6, 6, 0], "to": [10, 10, 6], "faces": { "up": { "texture": "#0" } } } ] } } }
}"##,
    )]);
    let c = compile(&root, "mymod", &yres::Refs::default()).unwrap();
    let m = output(&c, "assets/mymod/models/block/pipe.json");
    assert_eq!(m["textures"]["particle"], json!("minecraft:block/stone"));
    assert_eq!(m["render_type"], json!("minecraft:cutout"));
    assert_eq!(m["elements"][0]["rotation"]["origin"], json!([8, 8, 8]));
    assert_eq!(
        m["parent"],
        json!("minecraft:block/block"),
        "shown in a slot and held as a block"
    );
    let bs = output(&c, "assets/mymod/blockstates/pipe.json");
    assert_eq!(bs["multipart"].as_array().unwrap().len(), 7);
    assert_eq!(bs["multipart"][5]["apply"]["x"], json!(270));
    let src = c.source.unwrap().all();
    assert!(
        src.contains(".shape([6.0, 6.0, 6.0], [10.0, 10.0, 10.0])"),
        "{src}"
    );
    assert!(src.contains(".connects_to_neighbors()"), "{src}");
    assert!(src.contains(".connect_groups(&[\"mymod:pipe\"])"), "{src}");
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn sound_groups_are_the_games_or_a_mods_ydef() {
    let cube =
        r#"model: { parent: "cube_all", textures: { all: "@texture:minecraft:block/stone" } }"#;
    let crystal = r#"{ kind: "sound_group", id: "crystal", break: "crystal.break", step: "minecraft:block.glass.step",
  place: "crystal.place", hit: "crystal.hit", fall: "crystal.fall", pitch: 1.5 }"#;
    let a = format!(r#"{{ {cube}, block: {{ sound: "@sound_group:mymod:crystal" }} }}"#);
    let b = format!(r#"{{ {cube}, block: {{ sound: "amethyst" }} }}"#);
    let c = compile(
        &project(&[
            ("content/a.yblock", &a),
            ("content/b.yblock", &b),
            ("content/crystal.ydef", crystal),
        ]),
        "mymod",
        &yres::Refs::default(),
    )
    .unwrap();
    let src = c.source.unwrap().all();
    assert!(src.contains(r#"registry.register_sound_group(::yog_api::SoundGroup::new("mymod:crystal").sounds("mymod:crystal.break", "minecraft:block.glass.step", "mymod:crystal.place", "mymod:crystal.hit", "mymod:crystal.fall").volume(1.0).pitch(1.5));"#), "{src}");
    assert!(
        src.contains(r#".sound("mymod:crystal")"#)
            && src.contains(r#".sound("minecraft:amethyst")"#),
        "{src}"
    );

    let bad = format!(r#"{{ {cube}, block: {{ sound: "crystal" }} }}"#);
    let missing = format!(r#"{{ {cube}, block: {{ sound: "@sound_group:mymod:nope" }} }}"#);
    let e = compile(
        &project(&[("content/a.yblock", &bad), ("content/b.yblock", &missing)]),
        "mymod",
        &yres::Refs::default(),
    )
    .unwrap_err();
    assert!(
        e.contains("`crystal`: not one of the game's sound groups"),
        "{e}"
    );
    assert!(e.contains("no sound group mymod:nope"), "{e}");
}

#[test]
fn tools_are_tags_and_levels_are_definitions_a_mod_can_add() {
    let cube =
        r#"model: { parent: "cube_all", textures: { all: "@texture:minecraft:block/stone" } }"#;
    let mythril = r#"{ kind: "level", id: "mythril", ladder: "@ladder:minecraft:tool", above: "@level:minecraft:diamond" }"#;
    let block = format!(
        r#"{{ {cube}, block: {{ tool: ["pickaxe", "@tag:block:mymod:mineable/hammer"], tool_level: "@level:mymod:mythril" }} }}"#
    );
    let c = compile(
        &project(&[
            ("content/ore.yblock", &block),
            ("content/mythril.ydef", mythril),
        ]),
        "mymod",
        &yres::Refs::default(),
    )
    .unwrap();
    for tag in [
        "data/minecraft/tags/block/mineable/pickaxe.json",
        "data/mymod/tags/block/mineable/hammer.json",
        // Above diamond, on every version its own way.
        "data/minecraft/tags/block/needs_diamond_tool.json",
        "when/any/1.21/data/minecraft/tags/block/incorrect_for_diamond_tool.json",
        "when/fabric/1.20/data/fabric/tags/block/needs_tool_level_4.json",
        "when/forge/1.20/data/forge/tags/block/needs_netherite_tool.json",
    ] {
        assert!(
            c.tags
                .get(tag)
                .is_some_and(|ids| ids.contains_key("mymod:ore")),
            "{tag}: {:?}",
            c.tags
        );
    }
    // The game's levels by name.
    let iron = format!(r#"{{ {cube}, block: {{ tool: "pickaxe", tool_level: "iron" }} }}"#);
    let c = compile(
        &project(&[("content/ore.yblock", &iron)]),
        "mymod",
        &yres::Refs::default(),
    )
    .unwrap();
    assert!(c.tags["data/minecraft/tags/block/needs_iron_tool.json"].contains_key("mymod:ore"));
    assert_eq!(c.tags.len(), 2);
    // Mistakes.
    let bad = format!(r#"{{ {cube}, block: {{ tool: "hammer", tool_level: "mythril" }} }}"#);
    let e = compile(
        &project(&[("content/ore.yblock", &bad)]),
        "mymod",
        &yres::Refs::default(),
    )
    .unwrap_err();
    assert!(e.contains("block.tool: `hammer`: one of pickaxe, axe, shovel, hoe — or a tag of the mod's own tools"), "{e}");
    assert!(e.contains("block.tool_level: `mythril`: one of wood, stone, iron, diamond, netherite — or a level of the tool ladder"), "{e}");
    let volts = r#"{ kind: "ladder", id: "voltage" }"#;
    let lv = r#"{ kind: "level", id: "lv", ladder: "@ladder:mymod:voltage" }"#;
    let loop_ = r#"{ kind: "level", id: "hv", ladder: "@ladder:mymod:voltage", above: "@level:mymod:nope" }"#;
    let wrong = format!(
        r#"{{ {cube}, block: {{ tool_level: "@level:mymod:lv", drops: {{ item: "@item:mymod:nope" }} }} }}"#
    );
    let e = compile(
        &project(&[
            ("content/v.ydef", volts),
            ("content/lv.ydef", lv),
            ("content/hv.ydef", loop_),
            ("content/ore.yblock", &wrong),
        ]),
        "mymod",
        &yres::Refs::default(),
    )
    .unwrap_err();
    assert!(
        e.contains("level mymod:hv: above mymod:nope, which isn't a level of mymod:voltage"),
        "{e}"
    );
    assert!(
        e.contains(
            "block.tool_level: mymod:lv is a level of mymod:voltage, not of the tool ladder"
        ),
        "{e}"
    );
    assert!(
        e.contains("block.drops.item: no item mymod:nope in this mod"),
        "{e}"
    );
}

#[test]
fn reports_errors_with_lines() {
    // The schema first: every structural problem, with its line and path.
    let root = project(&[(
        "content/bad.yblock",
        "{\n  \"model\": {\n    \"textures\": { \"0\": \"@texture:minecraft:block/stone\" },\n    \"elements\": [ {\n      \"from\": [0, 0, 0], \"to\": [16, 16, 99],\n      \"rotation\": { \"axis\": \"q\", \"angle\": 30 },\n      \"faces\": { \"nort\": { \"texture\": \"#1\" } }\n    } ]\n  },\n  \"bogus\": 1\n}",
    )]);
    let e = compile(&root, "mymod", &yres::Refs::default()).unwrap_err();
    for want in [
        "content/bad.yblock:5:32: model.elements[0].to: 99 is out of range (-16..=32)",
        "content/bad.yblock:6:29: model.elements[0].rotation.axis: unknown `q` — did you mean `x`?",
        "content/bad.yblock:7:18: model.elements[0].faces.nort: unknown field `nort` — did you mean `north`?",
        "content/bad.yblock:10:3: bogus: unknown field `bogus`",
    ] {
        assert!(e.contains(want), "missing {want}\n{e}");
    }
    let _ = std::fs::remove_dir_all(root);
    // Then what it means: rotations Minecraft can't do, slots that aren't there.
    let root = project(&[(
        "content/bad.yblock",
        "{\n  model: {\n    textures: { \"0\": \"@texture:minecraft:block/stone\" },\n    elements: [ {\n      from: [0, 0, 0], to: [16, 16, 16],\n      rotation: { axis: \"x\", angle: 30 },\n      faces: { north: { texture: \"#1\" } }\n    } ]\n  },\n}",
    )]);
    let e = compile(&root, "mymod", &yres::Refs::default()).unwrap_err();
    for want in [
        "content/bad.yblock:6: model.elements[0].rotation.angle: 30 — Minecraft rotates elements by -45, -22.5, 0, 22.5 or 45 degrees",
        "content/bad.yblock:7: model.elements[0].faces.north.texture: `#1`: no such slot in textures",
    ] {
        assert!(e.contains(want), "missing {want}\n{e}");
    }
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn block_inventory_names_a_screen() {
    let block = r#"{ "block": { "inventory": "mymod:bench" }, "model": { "parent": "cube_all", "textures": { "all": "@texture:minecraft:block/stone" } } }"#;
    let root = project(&[
        ("content/bench.yblock", block),
        (
            "src/bench.yui",
            r#"<Screen id="mymod:bench" width="176" height="166" x:ViewModel="Vm"><Slot index="0"/></Screen>"#,
        ),
    ]);
    let c = compile(&root, "mymod", &yres::Refs::default()).unwrap();
    assert!(
        c.source
            .unwrap()
            .all()
            .contains(r#".inventory("mymod:bench")"#)
    );
    let _ = std::fs::remove_dir_all(root);

    let root = project(&[("content/bench.yblock", block)]);
    let e = compile(&root, "mymod", &yres::Refs::default()).unwrap_err();
    assert!(
        e.contains("no .yui <Screen id=\"mymod:bench\"> with <Slot>s"),
        "{e}"
    );
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn a_mod_has_loot_tables_of_its_own() {
    let table = r#"{ kind: "loot_table", id: "random_gem", add: [{ item: "@item:minecraft:diamond", count: [1, 3] }] }"#;
    let c = compile(
        &project(&[("content/random_gem.ydef", table)]),
        "mymod",
        &yres::Refs::default(),
    )
    .unwrap();
    let get = |p: &str| {
        c.outputs
            .iter()
            .find(|(q, _)| q == p)
            .map(|(_, b)| serde_json::from_slice::<Value>(b).unwrap())
    };
    assert_eq!(
        get("data/mymod/loot_table/random_gem.json").expect("the table")["pools"],
        json!([])
    );
    let src = c.source.unwrap().all();
    assert!(
        src.contains(r#"registry.add_loot_modifier_data("mymod:random_gem""#),
        "{src}"
    );
    let modifier = r#"{ kind: "loot_modifier", id: "more_gems", tables: ["@loot_table:minecraft:chests/simple_dungeon"], add: [{ item: "@item:minecraft:emerald", chance: 0.5 }] }"#;
    let c = compile(
        &project(&[("content/more_gems.ydef", modifier)]),
        "mymod",
        &yres::Refs::default(),
    )
    .unwrap();
    assert!(
        c.source
            .unwrap()
            .all()
            .contains(r#"registry.add_loot_modifier_data("mymod:more_gems""#)
    );
    let missing = r#"{ kind: "loot_modifier", id: "m", tables: ["@loot_table:minecraft:chests/nope"], add: [] }"#;
    assert!(
        compile(
            &project(&[("content/m.ydef", missing)]),
            "mymod",
            &yres::Refs::default()
        )
        .unwrap_err()
        .contains("chests/nope")
    );
    let bad = r#"{ kind: "loot_table", id: "minecraft:nope", add: [] }"#;
    assert!(
        compile(
            &project(&[("content/x.ydef", bad)]),
            "mymod",
            &yres::Refs::default()
        )
        .unwrap_err()
        .contains("the mod's own")
    );
}

#[test]
fn a_block_says_how_its_item_stacks() {
    let model =
        r#"model: { parent: "cube_all", textures: { all: "@texture:minecraft:block/stone" } }"#;
    let block = format!(
        r#"{{ item: {{ stack_size: 16, fire_resistant: true, fuel_ticks: 300 }}, {model} }}"#
    );
    let plain = format!(r#"{{ {model} }}"#);
    let c = compile(
        &project(&[
            ("content/log.yblock", &block),
            ("content/stone.yblock", &plain),
        ]),
        "mymod",
        &yres::Refs::default(),
    )
    .unwrap();
    let src = c.source.unwrap().all();
    assert!(src.contains(r#"registry.register_item(::yog_api::ItemDef::new("mymod:log").max_stack(16).fire_resistant().fuel(300));"#), "{src}");
    assert!(
        !src.contains(r#"ItemDef::new("mymod:stone")"#),
        "no item form of its own: the host's default"
    );
}

#[test]
fn blocks_and_items_join_their_tags() {
    let model =
        r#"model: { parent: "cube_all", textures: { all: "@texture:minecraft:block/stone" } }"#;
    let log = format!(r#"{{ tags: ["minecraft:logs", "edified_logs"], {model} }}"#);
    let staff = r#"{ tags: ["staves"], model: { parent: "generated", textures: { layer0: "@texture:minecraft:item/stick" } } }"#;
    let c = compile(
        &project(&[("content/log.yblock", &log), ("content/staff.yitem", staff)]),
        "mymod",
        &yres::Refs::default(),
    )
    .unwrap();
    for t in [
        "data/minecraft/tags/block/logs.json",
        "data/minecraft/tags/item/logs.json",
        "data/mymod/tags/block/edified_logs.json",
        "data/mymod/tags/item/edified_logs.json",
    ] {
        assert!(c.tags[t].contains_key("mymod:log"), "{t}");
    }
    assert!(c.tags["data/mymod/tags/item/staves.json"].contains_key("mymod:staff"));
    assert!(
        !c.tags.contains_key("data/mymod/tags/block/staves.json"),
        "an item has no block tag"
    );
}
