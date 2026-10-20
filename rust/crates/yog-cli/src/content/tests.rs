use super::*;

fn project(files: &[(&str, &str)]) -> PathBuf {
    use std::sync::atomic::{AtomicUsize, Ordering};
    static N: AtomicUsize = AtomicUsize::new(0);
    let dir = std::env::temp_dir().join(format!(
        "yog-content-test-{}-{}",
        std::process::id(),
        N.fetch_add(1, Ordering::SeqCst)
    ));
    let _ = std::fs::remove_dir_all(&dir);
    for (path, text) in files {
        let p = dir.join(path);
        std::fs::create_dir_all(p.parent().unwrap()).unwrap();
        std::fs::write(p, text).unwrap();
    }
    dir
}

/// 1×1 pictures named `names` in `resources/Images.yres` of the mod at `root`, and the refs its resources give.
fn pictures(root: &Path, names: &[&str]) -> yres::Refs {
    let png = yog_media::image::encode_png(&yog_media::image::Image { width: 1, height: 1, rgba: vec![255, 0, 0, 255] }).unwrap();
    let entries: Vec<yres::Entry> = names.iter().map(|n| yres::Entry::picture(n, png.clone())).collect();
    std::fs::create_dir_all(root.join("resources")).unwrap();
    std::fs::write(root.join("resources/Images.yres"), yres::write(&entries)).unwrap();
    yres::compile(root, "mymod").unwrap().refs
}

fn output(c: &Compiled, path: &str) -> Value {
    let (_, b) = c
        .outputs
        .iter()
        .find(|(p, _)| p == path)
        .unwrap_or_else(|| {
            panic!(
                "no {path} in {:?}",
                c.outputs.iter().map(|(p, _)| p).collect::<Vec<_>>()
            )
        });
    serde_json::from_slice(b).unwrap()
}

#[test]
fn compiles_a_cube_block() {
    let root = project(&[
        ("resources/block/ruby_ore.ytexture", r#"{ image: "@Images.block.ruby_ore" }"#),
        ("content/ruby.yitem", r#"{ model: { parent: "generated", textures: { layer0: "@texture:minecraft:item/emerald" } } }"#),
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
    assert_eq!(bs["variants"]["facing=north"], json!({ "model": "mymod:block/screen" }));
    assert_eq!(bs["variants"]["facing=west"]["y"], json!(270));
    assert!(c.source.unwrap().all().contains(".facing(\"horizontal\", \"toward\")"));
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn a_block_takes_its_materials_textures_and_layer() {
    let root = project(&[
        ("resources/block/glass.ymaterial", r#"{ textures: { all: "@texture:minecraft:block/glass", particle: "@texture:minecraft:block/sand" }, render_layer: "translucent" }"#),
        ("content/pane.yblock", r#"{ "model": { "parent": "cube_all", "material": "@material:mymod:block/glass", "textures": { "particle": "@texture:minecraft:block/stone" } } }"#),
    ]);
    let c = compile(&root, "mymod", &yres::Refs::default()).unwrap();
    let m = output(&c, "assets/mymod/models/block/pane.json");
    assert_eq!(m["textures"]["all"], json!("minecraft:block/glass"));
    assert_eq!(m["textures"]["particle"], json!("minecraft:block/stone"), "the model's own over the material's");
    assert_eq!(m["render_type"], json!("minecraft:translucent"));
    let missing = project(&[("content/x.yblock", r#"{ "model": { "parent": "cube_all", "material": "@material:mymod:block/none" } }"#)]);
    assert!(compile(&missing, "mymod", &yres::Refs::default()).unwrap_err().contains("material `mymod:block/none`"));
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
    assert_eq!(m["parent"], json!("minecraft:block/block"), "shown in a slot and held as a block");
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
    let cube = r#"model: { parent: "cube_all", textures: { all: "@texture:minecraft:block/stone" } }"#;
    let crystal = r#"{ kind: "sound_group", id: "crystal", break: "crystal.break", step: "minecraft:block.glass.step",
  place: "crystal.place", hit: "crystal.hit", fall: "crystal.fall", pitch: 1.5 }"#;
    let a = format!(r#"{{ {cube}, block: {{ sound: "@sound_group:mymod:crystal" }} }}"#);
    let b = format!(r#"{{ {cube}, block: {{ sound: "amethyst" }} }}"#);
    let c = compile(&project(&[("content/a.yblock", &a), ("content/b.yblock", &b), ("content/crystal.ydef", crystal)]), "mymod", &yres::Refs::default()).unwrap();
    let src = c.source.unwrap().all();
    assert!(src.contains(r#"registry.register_sound_group(::yog_api::SoundGroup::new("mymod:crystal").sounds("mymod:crystal.break", "minecraft:block.glass.step", "mymod:crystal.place", "mymod:crystal.hit", "mymod:crystal.fall").volume(1.0).pitch(1.5));"#), "{src}");
    assert!(src.contains(r#".sound("mymod:crystal")"#) && src.contains(r#".sound("minecraft:amethyst")"#), "{src}");

    let bad = format!(r#"{{ {cube}, block: {{ sound: "crystal" }} }}"#);
    let missing = format!(r#"{{ {cube}, block: {{ sound: "@sound_group:mymod:nope" }} }}"#);
    let e = compile(&project(&[("content/a.yblock", &bad), ("content/b.yblock", &missing)]), "mymod", &yres::Refs::default()).unwrap_err();
    assert!(e.contains("`crystal`: not one of the game's sound groups"), "{e}");
    assert!(e.contains("no sound group mymod:nope"), "{e}");
}

#[test]
fn tools_are_tags_and_levels_are_definitions_a_mod_can_add() {
    let cube = r#"model: { parent: "cube_all", textures: { all: "@texture:minecraft:block/stone" } }"#;
    let mythril = r#"{ kind: "level", id: "mythril", ladder: "@ladder:minecraft:tool", above: "@level:minecraft:diamond" }"#;
    let block = format!(r#"{{ {cube}, block: {{ tool: ["pickaxe", "@tag:block:mymod:mineable/hammer"], tool_level: "@level:mymod:mythril" }} }}"#);
    let c = compile(&project(&[("content/ore.yblock", &block), ("content/mythril.ydef", mythril)]), "mymod", &yres::Refs::default()).unwrap();
    for tag in [
        "data/minecraft/tags/block/mineable/pickaxe.json",
        "data/mymod/tags/block/mineable/hammer.json",
        // Above diamond, on every version its own way.
        "data/minecraft/tags/block/needs_diamond_tool.json",
        "when/any/1.21/data/minecraft/tags/block/incorrect_for_diamond_tool.json",
        "when/fabric/1.20/data/fabric/tags/block/needs_tool_level_4.json",
        "when/forge/1.20/data/forge/tags/block/needs_netherite_tool.json",
    ] {
        assert!(c.tags.get(tag).is_some_and(|ids| ids.contains_key("mymod:ore")), "{tag}: {:?}", c.tags);
    }
    // The game's levels by name.
    let iron = format!(r#"{{ {cube}, block: {{ tool: "pickaxe", tool_level: "iron" }} }}"#);
    let c = compile(&project(&[("content/ore.yblock", &iron)]), "mymod", &yres::Refs::default()).unwrap();
    assert!(c.tags["data/minecraft/tags/block/needs_iron_tool.json"].contains_key("mymod:ore"));
    assert_eq!(c.tags.len(), 2);
    // Mistakes.
    let bad = format!(r#"{{ {cube}, block: {{ tool: "hammer", tool_level: "mythril" }} }}"#);
    let e = compile(&project(&[("content/ore.yblock", &bad)]), "mymod", &yres::Refs::default()).unwrap_err();
    assert!(e.contains("block.tool: `hammer`: one of pickaxe, axe, shovel, hoe — or a tag of the mod's own tools"), "{e}");
    assert!(e.contains("block.tool_level: `mythril`: one of wood, stone, iron, diamond, netherite — or a level of the tool ladder"), "{e}");
    let volts = r#"{ kind: "ladder", id: "voltage" }"#;
    let lv = r#"{ kind: "level", id: "lv", ladder: "@ladder:mymod:voltage" }"#;
    let loop_ = r#"{ kind: "level", id: "hv", ladder: "@ladder:mymod:voltage", above: "@level:mymod:nope" }"#;
    let wrong = format!(r#"{{ {cube}, block: {{ tool_level: "@level:mymod:lv", drops: {{ item: "@item:mymod:nope" }} }} }}"#);
    let e = compile(&project(&[("content/v.ydef", volts), ("content/lv.ydef", lv), ("content/hv.ydef", loop_), ("content/ore.yblock", &wrong)]), "mymod", &yres::Refs::default()).unwrap_err();
    assert!(e.contains("level mymod:hv: above mymod:nope, which isn't a level of mymod:voltage"), "{e}");
    assert!(e.contains("block.tool_level: mymod:lv is a level of mymod:voltage, not of the tool ladder"), "{e}");
    assert!(e.contains("block.drops.item: no item mymod:nope in this mod"), "{e}");
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
        "content/bad.yblock:5: model.elements[0].to: 99 is out of range (-16..=32)",
        "content/bad.yblock:6: model.elements[0].rotation.axis: unknown `q` — did you mean `x`?",
        "content/bad.yblock:7: model.elements[0].faces.nort: unknown field `nort` — did you mean `north`?",
        "content/bad.yblock:10: bogus: unknown field `bogus`",
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
fn compiles_recipes() {
    let root = project(&[
        ("content/ruby.yitem", r#"{ "model": { "parent": "generated", "textures": { "layer0": "@texture:minecraft:item/emerald" } } }"#),
        (
            "content/recipes/ruby_block.yrecipe",
            r##"{ "type": "shaped", "pattern": ["RR", "RR"], "key": { "R": "@item:mymod:ruby" }, "result": "@item:minecraft:emerald_block" }"##,
        ),
        (
            "content/recipes/ruby.yrecipe",
            r##"{ "type": "shapeless", "ingredients": ["@item:minecraft:emerald", "@tag:item:c:dusts"], "result": "@item:mymod:ruby", "count": 2 }"##,
        ),
        (
            "content/recipes/smelt_ruby.yrecipe",
            r##"{ "type": "smelting", "ingredient": "@tag:item:c:ores", "result": "@item:mymod:ruby", "experience": 0.7, "$comment": "metadata goes", "_editor": { "x": 1 } }"##,
        ),
        (
            "content/recipes/cut.yrecipe",
            r##"{ "type": "stonecutting", "ingredient": ["@item:minecraft:stone", "@item:minecraft:andesite"], "result": "@item:mymod:ruby", "group": "rubies" }"##,
        ),
    ]);
    let c = compile(&root, "mymod", &yres::Refs::default()).unwrap();
    assert_eq!(c.recipes, 4);
    let src = c.source.clone().unwrap().all();
    for want in [
        r#"kind: ::yog_api::RecipeKind::Shaped { pattern: vec!["RR".into(), "RR".into()], key: vec![('R', vec!["mymod:ruby".into()])] }, result: "minecraft:emerald_block".into(), count: 1"#,
        r##"Shapeless { ingredients: vec![vec!["minecraft:emerald".into()], vec!["#c:dusts".into()]] }, result: "mymod:ruby".into(), count: 2"##,
        r##"Cooking { method: ::yog_api::Cooking::Smelting, ingredient: vec!["#c:ores".into()], experience: 0.7_f32, cook_time: 200 }"##,
        r##"Stonecutting { ingredient: vec!["minecraft:stone".into(), "minecraft:andesite".into()] }"##,
        r##"group: Some("rubies".into())"##,
    ] {
        assert!(src.contains(want), "missing {want}\n{src}");
    }
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn recipe_mistakes() {
    let bad = |recipe: &str| {
        let root = project(&[("content/r.yrecipe", recipe)]);
        let e = compile(&root, "mymod", &yres::Refs::default()).unwrap_err();
        let _ = std::fs::remove_dir_all(root);
        e
    };
    let e = bad(r#"{ "type": "shaped", "pattern": ["AB"], "key": { "A": "@item:minecraft:stick" }, "result": "@item:minecraft:stick" }"#);
    assert!(e.contains("pattern uses 'B'"), "{e}");
    let e = bad(r#"{ "type": "shapeless", "ingredients": ["stick"], "result": "@item:minecraft:stick" }"#);
    assert!(e.contains("ingredients[0]: `stick`: name it by a reference — `@item:stick`"), "{e}");
    let e = bad(r##"{ "type": "shapeless", "ingredients": ["#c:sand"], "result": "@item:minecraft:glass" }"##);
    assert!(e.contains("`#c:sand`: write a tag as `@tag:item:c:sand`"), "{e}");
    let e = bad(r##"{ "type": "smelting", "ingredient": "@item:minecraft:sand", "result": "@tag:item:c:glass" }"##);
    assert!(e.contains("result: expected a reference (`@item:`), found a tag"), "{e}");
    let e = bad(r#"{ "type": "shapeless", "ingredients": ["@item:minecraft:sand"], "result": "@item:mymod:nothing" }"#);
    assert!(e.contains("result: no item mymod:nothing in this mod"), "{e}");
    let e = bad(r#"{ "type": "shapeless", "ingredients": ["@item:minecraft:nope"], "result": "@item:minecraft:glass" }"#);
    assert!(e.contains("ingredients[0]: no item minecraft:nope in Minecraft"), "{e}");
    let e = bad(r#"{ "type": "shapeless", "pattern": ["A"], "ingredients": ["@item:minecraft:sand"], "result": "@item:minecraft:glass" }"#);
    assert!(e.contains("unknown field `pattern`"), "{e}");
    let e = bad(r#"{ "type": "smeltin", "ingredient": "@item:minecraft:sand", "result": "@item:minecraft:glass" }"#);
    assert!(e.contains("unknown `smeltin` — did you mean `smelting`?"), "{e}");
}

#[test]
fn block_inventory_names_a_screen() {
    let block = r#"{ "block": { "inventory": "mymod:bench" }, "model": { "parent": "cube_all", "textures": { "all": "@texture:minecraft:block/stone" } } }"#;
    let root = project(&[
        ("content/bench.yblock", block),
        ("src/bench.yui", r#"<Screen id="mymod:bench" width="176" height="166" x:ViewModel="Vm"><Slot index="0"/></Screen>"#),
    ]);
    let c = compile(&root, "mymod", &yres::Refs::default()).unwrap();
    assert!(c.source.unwrap().all().contains(r#".inventory("mymod:bench")"#));
    let _ = std::fs::remove_dir_all(root);

    let root = project(&[("content/bench.yblock", block)]);
    let e = compile(&root, "mymod", &yres::Refs::default()).unwrap_err();
    assert!(e.contains("no .yui <Screen id=\"mymod:bench\"> with <Slot>s"), "{e}");
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn compiles_a_book() {
    let book = r#"{
        "id": "guide",
        "name": "@Book.title",
        "categories": [{ "id": "mymod:basics", "name": "Basics", "icon": "@item:minecraft:book" }],
        "entries": [{
            "id": "mymod:ruby", "name": "@item:mymod:ruby", "category": "@category:mymod:basics",
            "pages": [
                { "type": "text", "text": "@@not a ref, {link=@entry:mymod:ruby|see}" },
                { "type": "spotlight", "item": "@item:mymod:ruby" },
                { "type": "entity", "entity": "@entity:minecraft:allay" }
            ]
        }]
    }"#;
    let ruby = r#"{ "name": "Ruby", "model": { "parent": "generated", "textures": { "layer0": "@texture:minecraft:item/emerald" } } }"#;
    let root = project(&[("content/guide.ybook", book), ("content/ruby.yitem", ruby)]);
    let mut refs = yres::Refs::default();
    refs.texts.insert("Book.title".into(), ("mymod.book.title".into(), "Guide".into()));
    let out = compile(&root, "mymod", &refs).unwrap();
    assert_eq!(out.books, 1);
    let src = out.source.unwrap().all();
    assert!(src.contains(r#"registry.register_book_data("mymod:guide", include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/generated/books/mymod__guide.ydata")));"#), "{src}");
    // The runtime reads the bytes by the book's schema; references come back as ids.
    assert_eq!(out.data_files[0].0, "books/mymod__guide.ydata");
    let data = yog_data::binary::decode(&out.data_files[0].1).unwrap();
    let text = yog_data::text::to_string(&data);
    assert!(text.contains("mymod.book.title") && text.contains("item.mymod.ruby"), "{text}");
    assert!(text.contains("not a ref") && text.contains("i18n"), "{text}");
    let read = yog_data::from_value::<yog_formats::book::Book>(&data).unwrap().value;
    assert_eq!(read.entries[0].category, "mymod:basics");
    assert!(matches!(&read.entries[0].pages[1], yog_formats::book::BookPage::Spotlight { item, .. } if item == "mymod:ruby"), "references become ids");

    // Bare ids, missing content and version-only ids are errors, each on its line.
    let bad = r#"{ "id": "x",
        "categories": [{ "id": "c", "name": "C" }],
        "entries": [{ "id": "e", "name": "E", "category": "c",
            "pages": [
                { "type": "spotlight", "item": "minecraft:stick" },
                { "type": "spotlight", "item": "@item:minecraft:grass" },
                { "type": "recipe", "recipe": "@recipe:mymod:nope" },
                { "type": "entity", "entity": "@entity:create:train" }
            ] }] }"#;
    let e = compile(&project(&[("content/bad.ybook", bad)]), "mymod", &yres::Refs::default()).unwrap_err();
    assert!(e.contains("bad.ybook:3: entries[0].category: `c`: name it by a reference — `@category:c`"), "{e}");
    assert!(e.contains("bad.ybook:5:") && e.contains("`@item:minecraft:stick`"), "{e}");
    let newest = super::refs::versions().last().expect("supported versions");
    assert!(e.contains(&format!("minecraft:grass isn't in Minecraft {newest} (the mod's datapack; it's in 1.20.1) — there it's minecraft:short_grass")), "{e}");
    assert!(e.contains("no recipe mymod:nope in this mod"), "{e}");
    assert!(e.contains("[external.create]"), "{e}");

    let bad = project(&[("content/bad.ybook", r#"{ "id": "x", "entries": [{ "pages": [{ "type": "nope" }] }] }"#)]);
    let e = compile(&bad, "mymod", &yres::Refs::default()).unwrap_err();
    assert!(e.contains("entries[0].pages[0].type: unknown page type `nope`"), "{e}");
    let bad = project(&[("content/bad.ybook", r#"{ "id": "x", "name": "X", "categories": [{ "id": "c", "name": "C" }] }"#)]);
    let e = compile(&bad, "mymod", &yres::Refs::default()).unwrap_err();
    assert!(e.contains("categories[0].id: `c`: write its namespace (`mymod:c`)"), "{e}");
}

#[test]
fn books_use_the_mods_own_page_types() {
    let ty = r#"<Component name="PatternPage" book_page="pattern">
        <Property name="name" type="book_text"/>
        <Property name="patterns" type="data" default="[]"/>
        <BookHeader text="{x:Bind name}"/>
        <Drawn drawer="mymod:pattern" h="64" data="{x:Bind patterns}"/>
    </Component>"#;
    let book = |page: &str| format!(r#"{{ id: "guide", name: "G", categories: [{{ id: "mymod:c", name: "C" }}],
        entries: [{{ id: "mymod:e", name: "E", category: "@category:mymod:c", pages: [{page}] }}] }}"#);
    let ok = book(r#"{ type: "mymod:pattern", name: "@item:minecraft:stick", patterns: [{ startdir: "EAST", signature: "qaq" }] }"#);
    let c = compile(&project(&[("src/ui/pattern_page.yui", ty), ("content/guide.ybook", &ok)]), "mymod", &yres::Refs::default()).unwrap();
    let b = yog_data::read_bytes::<yog_formats::book::Book>(&c.data_files[0].1).unwrap();
    assert_eq!(b.page_types.len(), 1, "the type goes with the book");
    let yog_formats::book::BookPage::Custom(p) = &b.entries[0].pages[0] else { panic!("{:?}", b.entries[0].pages[0]) };
    assert_eq!(p.fields.get("name").and_then(|n| n.as_str()), Some("item.minecraft.stick"), "texts resolve as the book's do");
    assert!(b.i18n);

    let unknown = book(r#"{ type: "mymod:pattern", name: "N", color: 3 }"#);
    let e = compile(&project(&[("src/ui/pattern_page.yui", ty), ("content/guide.ybook", &unknown)]), "mymod", &yres::Refs::default()).unwrap_err();
    assert!(e.contains("mymod:pattern has no field `color` (name, patterns)"), "{e}");
    let missing = book(r#"{ type: "mymod:nope" }"#);
    let e = compile(&project(&[("content/guide.ybook", &missing)]), "mymod", &yres::Refs::default()).unwrap_err();
    assert!(e.contains("no page type mymod:nope"), "{e}");
}

#[test]
fn tools_and_armor_of_the_games_materials_or_the_mods() {
    let model = r#"model: { parent: "generated", textures: { layer0: "@texture:minecraft:item/iron_pickaxe" } }"#;
    let level = r#"{ kind: "level", id: "mythril", ladder: "@ladder:minecraft:tool", above: "@level:minecraft:diamond" }"#;
    let tool = r#"{ kind: "tool_material", id: "mythril", durability: 1800, speed: 10, attack_damage: 3, enchantability: 18,
        level: "@level:mymod:mythril", repair: "@tag:item:c:ingots" }"#;
    let armor = r#"{ kind: "armor_material", id: "mythril", durability: 30, defense: { head: 3, chest: 8, legs: 6, feet: 3 },
        enchantability: 18, equip_sound: "minecraft:item.armor.equip_diamond", repair: "@item:minecraft:diamond" }"#;
    let pick = format!(r#"{{ {model}, item: {{ tool: {{ kind: "pickaxe", material: "@tool_material:mymod:mythril" }} }} }}"#);
    let helm = format!(r#"{{ {model}, item: {{ armor: {{ slot: "head", material: "iron" }} }} }}"#);
    let files = [("content/mythril.ydef", level), ("content/mythril_tools.ydef", tool), ("content/mythril_armor.ydef", armor), ("content/pick.yitem", pick.as_str()), ("content/helm.yitem", helm.as_str())];
    let c = compile(&project(&files), "mymod", &yres::Refs::default()).unwrap();
    let src = c.source.unwrap().all();
    assert!(src.contains(r#".tool("pickaxe", "mymod:mythril", 1.0, -2.8)"#), "the kind's attack: {src}");
    assert!(src.contains(r#".armor("head", "minecraft:iron")"#), "{src}");
    assert!(src.contains(r##"level: "mymod:mythril".into(), repair: "#c:ingots".into(), level_index: 4, incorrect: "minecraft:incorrect_for_netherite_tool".into()"##), "above diamond: netherite's on both versions: {src}");
    assert!(src.contains(r#"registry.register_armor_material(::yog_api::ArmorMaterial { id: "mymod:mythril".into()"#) && src.contains(r#"texture: "mymod:mythril".into()"#), "{src}");

    let bad = format!(r#"{{ {model}, item: {{ tool: {{ kind: "spoon", material: "bronze" }} }} }}"#);
    let e = compile(&project(&[("content/bad.yitem", &bad)]), "mymod", &yres::Refs::default()).unwrap_err();
    assert!(e.contains("`spoon`: one of pickaxe, axe, shovel, hoe, sword") && e.contains("`bronze`: one of the game's (wood, stone, iron, diamond, gold, netherite)"), "{e}");
    let missing = format!(r#"{{ {model}, item: {{ armor: {{ slot: "feet", material: "@armor_material:mymod:nope" }} }} }}"#);
    let e = compile(&project(&[("content/m.yitem", &missing)]), "mymod", &yres::Refs::default()).unwrap_err();
    assert!(e.contains("no armor material mymod:nope"), "{e}");
}

#[test]
fn data_naming_renamed_content_gets_each_versions_name() {
    let model = r#"model: { parent: "cube_all", textures: { all: "@texture:minecraft:block/stone" } }"#;
    let block = format!(r#"{{ {model}, block: {{ drops: {{ item: "@item:minecraft:short_grass" }} }} }}"#);
    let c = compile(&project(&[("content/tuft.yblock", &block)]), "mymod", &yres::Refs::default()).unwrap();
    let loot = |p: &str| c.outputs.iter().find(|(q, _)| q == p).map(|(_, b)| String::from_utf8_lossy(b).to_string());
    assert!(loot("data/mymod/loot_table/blocks/tuft.json").is_some_and(|l| l.contains("minecraft:short_grass")), "the datapack's name");
    let old = loot("when/any/1.20.1/data/mymod/loot_table/blocks/tuft.json").expect("1.20.1's overlay");
    assert!(old.contains("minecraft:grass") && !old.contains("short_grass"), "{old}");
    let crafter = format!(r#"{{ {model}, block: {{ drops: {{ item: "@item:minecraft:crafter" }} }} }}"#);
    let c = compile(&project(&[("content/c.yblock", &crafter)]), "mymod", &yres::Refs::default()).unwrap();
    assert!(c.warnings.iter().any(|w| w.contains("minecraft:crafter, which Minecraft 1.20.1 hasn't got")), "{:?}", c.warnings);
}

#[test]
fn ores_become_features_in_biomes_on_every_loader() {
    let model = r#"model: { parent: "cube_all", textures: { all: "@texture:minecraft:block/stone" } }"#;
    let block = format!(r#"{{ {model} }}"#);
    let ore = r#"{ kind: "ore", id: "mythril_ore", targets: [{ replaces: "stone", block: "@block:mymod:mythril_ore" },
        { replaces: "@block:minecraft:basalt", block: "@block:mymod:mythril_ore" }], size: 6, per_chunk: 4,
        height: { min: -48, max: 16, shape: "triangle" } }"#;
    let c = compile(&project(&[("content/mythril_ore.yblock", &block), ("content/mythril_ore.ydef", ore)]), "mymod", &yres::Refs::default()).unwrap();
    let get = |p: &str| c.outputs.iter().find(|(q, _)| q == p).map(|(_, b)| serde_json::from_slice::<Value>(b).unwrap()).unwrap_or_else(|| panic!("no {p}"));
    let cf = get("data/mymod/worldgen/configured_feature/mythril_ore.json");
    assert_eq!(cf["config"]["targets"][0]["target"]["tag"], "minecraft:stone_ore_replaceables");
    assert_eq!(cf["config"]["targets"][1]["target"]["block"], "minecraft:basalt");
    let pf = get("data/mymod/worldgen/placed_feature/mythril_ore.json");
    assert_eq!(pf["placement"][2]["height"]["type"], "minecraft:trapezoid");
    assert_eq!(get("when/neoforge/1.21/data/mymod/neoforge/biome_modifier/mythril_ore.json")["type"], "neoforge:add_features");
    assert_eq!(get("when/forge/any/data/mymod/forge/biome_modifier/mythril_ore.json")["biomes"], "#minecraft:is_overworld");
    let src = c.source.unwrap().all();
    assert!(src.contains(r#"registry.add_biome_feature("mymod:mythril_ore", "minecraft:is_overworld", "underground_ores");"#), "{src}");
    let bad = r#"{ kind: "ore", id: "x", targets: [{ replaces: "granite", block: "@block:mymod:nope" }], size: 6, per_chunk: 4, height: { min: 0, max: 10 } }"#;
    let e = compile(&project(&[("content/x.ydef", bad)]), "mymod", &yres::Refs::default()).unwrap_err();
    assert!(e.contains("`granite`: one of stone, deepslate, netherrack, end_stone"), "{e}");
    let missing = r#"{ kind: "ore", id: "x", targets: [{ replaces: "stone", block: "@block:mymod:nope" }], size: 6, per_chunk: 4, height: { min: 0, max: 10 } }"#;
    let e = compile(&project(&[("content/x.ydef", missing)]), "mymod", &yres::Refs::default()).unwrap_err();
    assert!(e.contains("no block mymod:nope"), "{e}");
}

#[test]
fn advancements_are_definitions_for_every_version() {
    let root = r#"{ kind: "advancement", id: "root", icon: "@item:minecraft:amethyst_shard", title: "Gems",
        background: "minecraft:textures/block/calcite.png", criteria: [{ has_item: "@tag:item:yog:gems/amethyst" }] }"#;
    let lore = r#"{ kind: "advancement", id: "lore/one", parent: "@advancement:mymod:root", icon: "@item:minecraft:paper",
        title: "@lang:advancement.mymod.lore.one", hidden: true, criteria: [{ granted: true }] }"#;
    let c = compile(&project(&[("content/root.ydef", root), ("content/lore.ydef", lore)]), "mymod", &yres::Refs::default()).unwrap();
    let get = |p: &str| c.outputs.iter().find(|(q, _)| q == p).map(|(_, b)| serde_json::from_slice::<Value>(b).unwrap()).unwrap_or_else(|| panic!("no {p}"));
    let new = get("data/mymod/advancement/root.json");
    assert_eq!(new["display"]["icon"]["id"], "minecraft:amethyst_shard");
    assert_eq!(new["criteria"]["c0"]["conditions"]["items"][0]["items"], "#yog:gems/amethyst");
    assert_eq!(new["display"]["title"]["translate"], "advancement.mymod.root");
    let old = get("when/any/1.20.1/data/mymod/advancement/root.json");
    assert_eq!(old["display"]["icon"]["item"], "minecraft:amethyst_shard");
    assert_eq!(old["criteria"]["c0"]["conditions"]["items"][0]["tag"], "yog:gems/amethyst");
    let one = get("data/mymod/advancement/lore/one.json");
    assert_eq!((one["parent"].as_str(), one["criteria"]["c0"]["trigger"].as_str()), (Some("mymod:root"), Some("minecraft:impossible")));
    assert_eq!(c.lang[&("mymod".to_string(), yres::NEUTRAL_LOCALE.to_string())]["advancement.mymod.root"], "Gems");
    // The code names them by constant.
    let src = &c.source.as_ref().unwrap().all();
    assert!(src.contains("pub const LORE_ONE: &'static str = \"mymod:lore/one\";"), "{src}");
    let bad = r#"{ kind: "advancement", id: "x", icon: "@item:minecraft:paper", title: "X", criteria: [{ granted: true, used_item: "@item:minecraft:paper" }] }"#;
    assert!(compile(&project(&[("content/x.ydef", bad)]), "mymod", &yres::Refs::default()).unwrap_err().contains("one of granted"));
}

#[test]
fn a_mod_has_loot_tables_of_its_own() {
    let table = r#"{ kind: "loot_table", id: "random_gem", add: [{ item: "@item:minecraft:diamond", count: [1, 3] }] }"#;
    let c = compile(&project(&[("content/random_gem.ydef", table)]), "mymod", &yres::Refs::default()).unwrap();
    let get = |p: &str| c.outputs.iter().find(|(q, _)| q == p).map(|(_, b)| serde_json::from_slice::<Value>(b).unwrap());
    assert_eq!(get("data/mymod/loot_table/random_gem.json").expect("the table")["pools"], json!([]));
    let src = c.source.unwrap().all();
    assert!(src.contains(r#"registry.add_loot_modifier_data("mymod:random_gem""#), "{src}");
    let modifier = r#"{ kind: "loot_modifier", id: "more_gems", tables: ["@loot_table:minecraft:chests/simple_dungeon"], add: [{ item: "@item:minecraft:emerald", chance: 0.5 }] }"#;
    let c = compile(&project(&[("content/more_gems.ydef", modifier)]), "mymod", &yres::Refs::default()).unwrap();
    assert!(c.source.unwrap().all().contains(r#"registry.add_loot_modifier_data("mymod:more_gems""#));
    let missing = r#"{ kind: "loot_modifier", id: "m", tables: ["@loot_table:minecraft:chests/nope"], add: [] }"#;
    assert!(compile(&project(&[("content/m.ydef", missing)]), "mymod", &yres::Refs::default()).unwrap_err().contains("chests/nope"));
    let bad = r#"{ kind: "loot_table", id: "minecraft:nope", add: [] }"#;
    assert!(compile(&project(&[("content/x.ydef", bad)]), "mymod", &yres::Refs::default()).unwrap_err().contains("the mod's own"));
}

#[test]
fn a_block_says_how_its_item_stacks() {
    let model = r#"model: { parent: "cube_all", textures: { all: "@texture:minecraft:block/stone" } }"#;
    let block = format!(r#"{{ item: {{ stack_size: 16, fire_resistant: true, fuel_ticks: 300 }}, {model} }}"#);
    let plain = format!(r#"{{ {model} }}"#);
    let c = compile(&project(&[("content/log.yblock", &block), ("content/stone.yblock", &plain)]), "mymod", &yres::Refs::default()).unwrap();
    let src = c.source.unwrap().all();
    assert!(src.contains(r#"registry.register_item(::yog_api::ItemDef::new("mymod:log").max_stack(16).fire_resistant().fuel(300));"#), "{src}");
    assert!(!src.contains(r#"ItemDef::new("mymod:stone")"#), "no item form of its own: the host's default");
}

#[test]
fn blocks_and_items_join_their_tags() {
    let model = r#"model: { parent: "cube_all", textures: { all: "@texture:minecraft:block/stone" } }"#;
    let log = format!(r#"{{ tags: ["minecraft:logs", "edified_logs"], {model} }}"#);
    let staff = r#"{ tags: ["staves"], model: { parent: "generated", textures: { layer0: "@texture:minecraft:item/stick" } } }"#;
    let c = compile(&project(&[("content/log.yblock", &log), ("content/staff.yitem", staff)]), "mymod", &yres::Refs::default()).unwrap();
    for t in ["data/minecraft/tags/block/logs.json", "data/minecraft/tags/item/logs.json", "data/mymod/tags/block/edified_logs.json", "data/mymod/tags/item/edified_logs.json"] {
        assert!(c.tags[t].contains_key("mymod:log"), "{t}");
    }
    assert!(c.tags["data/mymod/tags/item/staves.json"].contains_key("mymod:staff"));
    assert!(!c.tags.contains_key("data/mymod/tags/block/staves.json"), "an item has no block tag");
}

#[test]
fn trees_are_definitions() {
    let model = r#"model: { parent: "cube_all", textures: { all: "@texture:minecraft:block/stone" } }"#;
    let block = format!(r#"{{ {model} }}"#);
    let tree = r#"{ kind: "tree", id: "glass_tree", min_clipped_height: 6,
        trunk: { blocks: [{ block: "@block:mymod:log", weight: 8 }, { block: "@block:minecraft:oak_log" }], shape: "fancy", base_height: 5, random_a: 5, random_b: 3 },
        leaves: { blocks: [{ block: "@block:mymod:leaves" }], shape: "fancy", radius: 1, offset: 5, height: 5 } }"#;
    let c = compile(&project(&[("content/log.yblock", &block), ("content/leaves.yblock", &block), ("content/glass_tree.ydef", tree)]), "mymod", &yres::Refs::default()).unwrap();
    let get = |p: &str| c.outputs.iter().find(|(q, _)| q == p).map(|(_, b)| serde_json::from_slice::<Value>(b).unwrap()).unwrap_or_else(|| panic!("no {p}"));
    let f = &get("data/mymod/worldgen/configured_feature/glass_tree.json")["config"];
    assert_eq!(f["trunk_placer"]["type"], "minecraft:fancy_trunk_placer");
    assert_eq!(f["trunk_provider"]["entries"][0]["weight"], 8);
    assert_eq!(f["foliage_provider"]["state"]["Name"], "mymod:leaves");
    assert_eq!((f["foliage_placer"]["height"].as_u64(), f["minimum_size"]["min_clipped_height"].as_u64()), (Some(5), Some(6)));
    let bad = r#"{ kind: "tree", id: "t", trunk: { blocks: [{ block: "@block:mymod:nope" }], shape: "straight", base_height: 4 },
        leaves: { blocks: [{ block: "@block:minecraft:oak_leaves" }], shape: "blob", radius: 2 } }"#;
    let e = compile(&project(&[("content/t.ydef", bad)]), "mymod", &yres::Refs::default()).unwrap_err();
    assert!(e.contains("no block mymod:nope"), "{e}");
}

#[test]
fn what_a_version_or_loader_hasnt_got_keeps_a_recipe_out_of_it() {
    let recipe = |key: &str| format!(r#"{{ type: "shaped", pattern: ["D"], key: {{ D: {key} }}, result: "@item:minecraft:stick" }}"#);
    let check = |key: &str| {
        let c = compile(&project(&[("content/r.yrecipe", &recipe(key))]), "mymod", &yres::Refs::default());
        c.map(|c| c.warnings.iter().filter(|w| w.contains("left out")).cloned().collect::<Vec<_>>().join("\n"))
    };
    let w = check(r#""@tag:item:minecraft:music_discs""#).unwrap();
    assert!(w.contains("left out on Minecraft 1.21.1, Minecraft 26.2: #minecraft:music_discs isn't there — @tag:item:yog:music_discs is on every version and loader"), "{w}");
    assert_eq!(check(r#""@tag:item:yog:music_discs""#).unwrap(), "");
    assert_eq!(check(r#"["@tag:item:c:iron_ingots", "@item:minecraft:iron_ingot"]"#).unwrap(), "", "an alternative that's everywhere");
    let w = check(r#""@tag:item:c:iron_ingots""#).unwrap();
    assert!(w.contains("left out on forge 1.20.1, neoforge 1.20.1, Minecraft 1.21.1") && w.contains("@tag:item:yog:ingots/iron"), "{w}");
    let e = check(r#""@tag:item:c:nope""#).unwrap_err();
    assert!(e.contains("no item tag c:nope in Minecraft or its loaders"), "{e}");
    let e = check(r#""@tag:item:yog:nope""#).unwrap_err();
    assert!(e.contains("no item tag yog:nope among Yog's common tags"), "{e}");
    assert!(check(r#""@tag:item:othermod:anything""#).is_ok(), "another mod's tags aren't known here");
    let block = r#"{ tags: ["yog:planks", "yog:nope"], model: { parent: "cube_all", textures: { all: "@texture:minecraft:block/stone" } } }"#;
    let e = compile(&project(&[("content/p.yblock", block)]), "mymod", &yres::Refs::default()).unwrap_err();
    assert!(e.contains("tags[1]: no block tag yog:nope") && !e.contains("yog:planks"), "{e}");
}

#[test]
fn tags_and_damage_types_are_definitions() {
    let tag = r#"{ kind: "tag", of: "entity", id: "sticky_teleporters", values: ["@entity:minecraft:pig", "@tag:entity:c:bosses"],
        optional: ["@entity:create:seat"] }"#;
    let logs = r#"{ kind: "tag", of: "block", id: "minecraft:logs", values: ["@block:minecraft:stone"] }"#;
    let dmg = r#"{ kind: "damage_type", id: "overcast", scaling: "never", tags: ["minecraft:bypasses_armor", "bypasses_effects"] }"#;
    let c = compile(&project(&[("content/a.ydef", tag), ("content/b.ydef", logs), ("content/c.ydef", dmg)]), "mymod", &yres::Refs::default()).unwrap();
    let sticky = &c.tags["data/mymod/tags/entity_type/sticky_teleporters.json"];
    assert_eq!(sticky.iter().map(|(k, v)| (k.as_str(), *v)).collect::<Vec<_>>(), vec![("#c:bosses", false), ("create:seat", false), ("minecraft:pig", true)], "c:bosses isn't on every loader: where it isn't, the tag still loads");
    assert!(c.tags["data/minecraft/tags/block/logs.json"].contains_key("minecraft:stone"));
    let get = |p: &str| c.outputs.iter().find(|(q, _)| q == p).map(|(_, b)| serde_json::from_slice::<Value>(b).unwrap()).unwrap_or_else(|| panic!("no {p}"));
    let d = get("data/mymod/damage_type/overcast.json");
    assert_eq!((d["message_id"].as_str(), d["scaling"].as_str()), (Some("mymod.overcast"), Some("never")));
    for t in ["data/minecraft/tags/damage_type/bypasses_armor.json", "data/minecraft/tags/damage_type/bypasses_effects.json"] {
        assert!(c.tags[t].contains_key("mymod:overcast"), "{t}");
    }
    let bad = r#"{ kind: "tag", of: "entity", id: "x", values: ["@entity:minecraft:nope", "@block:minecraft:stone"] }"#;
    let e = compile(&project(&[("content/x.ydef", bad)]), "mymod", &yres::Refs::default()).unwrap_err();
    assert!(e.contains("nope") && e.contains("`@block:minecraft:stone` in a tag of entity"), "{e}");
}

#[test]
fn a_dimension_and_its_terrain_go_to_the_runtime_as_data() {
    let dim = r#"{ kind: "dimension", id: "flatlands", dimension_type: { has_sky_light: true }, biome: "@biome:minecraft:desert",
        terrain: { seed: 3, base_height: 70, noise: [{ scale: 80, amplitude: 6 }], fill: "@block:minecraft:sandstone",
            layers: [{ block: "@block:minecraft:sand", depth: 3 }] } }"#;
    let c = compile(&project(&[("content/flatlands.ydef", dim)]), "mymod", &yres::Refs::default()).unwrap();
    let (path, data) = c.data_files.iter().find(|(p, _)| p.starts_with("dimensions/")).expect("the dimension's data");
    assert_eq!(path, "dimensions/mymod__flatlands.ydata");
    let v = yog_data::binary::decode(data).unwrap();
    assert_eq!(v.get("biome"), Some(&yog_data::Value::from("minecraft:desert")), "a plain id: the hosts read it");
    assert_eq!(v.get("generator_type"), Some(&yog_data::Value::from("mymod:flatlands")));
    let t = yog_data::read_value::<yog_worldgen::Terrain>(v.get("terrain").unwrap()).unwrap();
    assert_eq!((t.base_height, t.fill.as_str()), (70, "minecraft:sandstone"));
    let src = c.source.unwrap().all();
    assert!(src.contains(r#"registry.register_dimension_data("mymod:flatlands", include_bytes!("#), "{src}");
    let bad = r#"{ kind: "dimension", id: "x", terrain: { fill: "@block:minecraft:nope" } }"#;
    let e = compile(&project(&[("content/x.ydef", bad)]), "mymod", &yres::Refs::default()).unwrap_err();
    assert!(e.contains("no block minecraft:nope"), "{e}");
}

#[test]
fn content_for_other_mods_is_registered_only_with_them() {
    let model = r#"model: { parent: "generated", textures: { layer0: "@texture:minecraft:item/stick" } }"#;
    let item = format!(r#"{{ requires: ["pehkui", "create"], {model} }}"#);
    let recipe = r#"{ requires: ["pehkui"], type: "shapeless", ingredients: ["@item:minecraft:stick"], result: "@item:minecraft:stick" }"#;
    let c = compile(&project(&[("content/shrink.yitem", &item), ("content/r.yrecipe", recipe)]), "mymod", &yres::Refs::default()).unwrap();
    let src = c.source.unwrap().all();
    assert!(src.contains(r#"if ::yog_api::mod_loaded("pehkui") && ::yog_api::mod_loaded("create") { registry.register_item(Items::shrink()); }"#), "{src}");
    assert!(src.contains(r#"if ::yog_api::mod_loaded("pehkui") { registry.add_recipe("#), "{src}");
}

#[test]
fn books_lay_out_the_mods_own_recipe_kinds() {
    let recipe = r#"{ type: "mymod:infusion", slots: { core: ["@item:minecraft:diamond"] }, result: "@item:minecraft:emerald" }"#;
    let book = r#"{ id: "guide", name: "G", categories: [{ id: "mymod:c", name: "C" }],
        entries: [{ id: "mymod:e", name: "E", category: "@category:mymod:c",
            pages: [{ type: "recipe", recipe: "@recipe:mymod:infusion" }] }] }"#;
    let kind = r#"{ kind: "recipe_kind", id: "infusion", slots: { core: { required: true } } }"#;
    let layout = r#"<Component name="InfusionLayout" recipe_layout="infusion" width="96" height="24">
            <RecipeSlot slot="core" x="4" y="4" frame="true"/>
            <ItemSlot item="minecraft:beacon" x="40" y="4"/>
            <RecipeResult x="76" y="4" frame="true"/>
        </Component>"#;
    let files = [("content/infusion.yrecipe", recipe), ("content/guide.ybook", book), ("content/infusion.ydef", kind)];
    let e = compile(&project(&files), "mymod", &yres::Refs::default()).unwrap_err();
    assert!(e.contains("entries[0].pages[0].recipe: no layout for recipe kind mymod:infusion"), "{e}");
    let c = compile(&project(&[files[0], files[1], files[2], ("src/ui/infusion.yui", layout)]), "mymod", &yres::Refs::default()).unwrap();
    let book = yog_data::read_bytes::<yog_formats::book::Book>(&c.data_files[0].1).unwrap();
    assert_eq!(book.layouts[0].kinds, ["mymod:infusion"], "the book carries the mod's layouts");
    // What a layout names is checked: the kinds, a mod kind's slots, the stations.
    let bad = r#"<Component recipe_layout="create:mixing infusion" width="1" height="1">
            <RecipeSlot slot="ring"/><ItemSlot item="minecraft:nope"/></Component>"#;
    let e = compile(&project(&[files[2], ("src/ui/bad.yui", bad)]), "mymod", &yres::Refs::default()).unwrap_err();
    assert!(e.contains("`create:mixing`: one of shaped"), "{e}");
    assert!(e.contains("mymod:infusion has no slot `ring` — core"), "{e}");
    assert!(e.contains("<ItemSlot item=\"minecraft:nope\">: no item minecraft:nope in Minecraft"), "{e}");
}

#[test]
fn compiles_a_recipe_of_the_mods_own_kind() {
    let kind = r#"{ kind: "recipe_kind", id: "infusion", slots: { core: { required: true }, ring: { max: 2 } }, values: { time: "number" } }"#;
    let r = r##"{ type: "mymod:infusion", slots: { core: ["@item:minecraft:diamond"], ring: [["@item:minecraft:coal", "@item:minecraft:charcoal"], "@tag:item:c:dusts"] },
                 values: { time: 200 }, result: "@item:minecraft:emerald", count: 2 }"##;
    let c = compile(&project(&[("content/infusion.ydef", kind), ("content/infusion.yrecipe", r)]), "mymod", &yres::Refs::default()).unwrap();
    let src = c.source.unwrap().all();
    assert!(src.contains("registry.add_recipe_json(\"mymod:infusion\""), "{src}");
    assert!(src.contains(r#"\"type\":\"mymod:infusion\""#) && src.contains(r#"\"time\":200"#), "{src}");

    // Against its kind: slots, how many, values and their types.
    let bad = r##"{ type: "mymod:infusion", slots: { ring: ["@item:minecraft:coal", "@item:minecraft:coal", "@item:minecraft:coal"], nope: ["@item:minecraft:coal"] },
                   values: { time: "soon", heat: 3 }, result: "@item:minecraft:emerald" }"##;
    let e = compile(&project(&[("content/infusion.ydef", kind), ("content/bad.yrecipe", bad)]), "mymod", &yres::Refs::default()).unwrap_err();
    for want in ["slots.ring: at most 2 ingredient(s), found 3", "slots.nope: mymod:infusion has no slot `nope` (core, ring)", "slots: mymod:infusion needs `core`", "values.time: a number", "values.heat: mymod:infusion has no value `heat` (time)"] {
        assert!(e.contains(want), "{want}: {e}");
    }
    let undeclared = r#"{ "type": "mymod:infusion", "slots": {}, "result": "@item:minecraft:emerald" }"#;
    let e = compile(&project(&[("content/u.yrecipe", undeclared)]), "mymod", &yres::Refs::default()).unwrap_err();
    assert!(e.contains("type `mymod:infusion`: no such kind — declare it: content/infusion.ydef"), "{e}");
    let bad = r#"{ "type": "mymod:infusion", "ingredient": "@item:minecraft:stone", "result": "@item:minecraft:emerald" }"#;
    let e = compile(&project(&[("content/infusion.ydef", kind), ("content/bad.yrecipe", bad)]), "mymod", &yres::Refs::default()).unwrap_err();
    assert!(e.contains("unknown field `ingredient`") && e.contains("missing `slots`"), "{e}");
    // Another mod's kind isn't the mod's own: not a Yog recipe type.
    let other = r#"{ "type": "create:mixing", "slots": {}, "result": "@item:minecraft:emerald" }"#;
    let e = compile(&project(&[("content/o.yrecipe", other)]), "mymod", &yres::Refs::default()).unwrap_err();
    assert!(e.contains("type `create:mixing`: one of shaped"), "{e}");
}

#[test]
fn compiles_an_item_with_resource_refs() {
    let root = project(&[
        (
            "content/items/ruby.yitem",
            r#"{ "name": "@Items.ruby", "tooltip": "@Strings.ruby_tip",
             "item": { "stack_size": 16, "food": { "nutrition": 4, "saturation": 0.3 } },
             "model": { "parent": "generated", "textures": { "layer0": "@texture:mymod:item/ruby" } } }"#,
        ),
        ("resources/item/ruby.ytexture", r#"{ image: "@Images.item.ruby" }"#),
    ]);
    let mut refs = pictures(&root, &["item.ruby"]);
    refs.texts.insert(
        "Items.ruby".into(),
        ("item.mymod.ruby".into(), "Ruby".into()),
    );
    refs.texts.insert(
        "Strings.ruby_tip".into(),
        ("mymod.ruby_tip".into(), "Shiny".into()),
    );
    let c = compile(&root, "mymod", &refs).unwrap();
    assert_eq!(
        output(&c, "assets/mymod/models/item/ruby.json"),
        json!({ "parent": "minecraft:item/generated", "textures": { "layer0": "mymod:item/ruby" } })
    );
    let src = c.source.clone().unwrap().all();
    assert!(src.contains(".max_stack(16)"), "{src}");
    assert!(
        src.contains(".food(::yog_api::FoodDef::new(4, 0.3))"),
        "{src}"
    );
    // The .yres lang file gets the tooltip under the key the game reads.
    let mut outputs = vec![(
        "assets/mymod/lang/ru_ru.json".to_string(),
        r#"{"item.mymod.ruby": "Рубин", "mymod.ruby_tip": "Блестит"}"#
            .as_bytes()
            .to_vec(),
    )];
    merge_outputs(&root, &mut outputs, c).unwrap();
    let ru: Value = serde_json::from_slice(&outputs[0].1).unwrap();
    assert_eq!(ru["item.mymod.ruby.desc"], json!("Блестит"));
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn compiles_a_potion_item_model_without_registering_a_second_item() {
    let root = project(&[(
        "content/items/potion.yitem",
        r#"{ "item": { "potion": { "effects": [] } },
             "model": { "parent": "minecraft:item/potion",
                        "textures": { "layer0": "@texture:minecraft:item/potion_overlay" } } }"#,
    )]);
    let c = compile(&root, "mymod", &yres::Refs::default()).unwrap();
    assert_eq!(
        output(&c, "assets/mymod/models/item/potion.json"),
        json!({
            "parent": "minecraft:item/potion",
            "textures": { "layer0": "minecraft:item/potion_overlay" }
        })
    );
    assert!(
        !c.outputs.iter().any(|(path, _)| path == "assets/mymod/items/potion.json"),
        "a potion registry entry must not create a second item definition"
    );
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn namespace_needed_for_odd_mod_ids() {
    let root = project(&[(
        "content/x.yitem",
        r#"{ "model": { "parent": "generated", "textures": { "layer0": "@texture:minecraft:item/apple" } } }"#,
    )]);
    let e = compile(&root, "Example-Mod", &yres::Refs::default()).unwrap_err();
    assert!(e.contains("give the namespace"), "{e}");
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn body_files_compile_and_register() {
    let model = r#"{ "texture": "@texture:minecraft:entity/elytra", "bones": [
        { "name": "wing", "pivot": [5, 0, 0], "cubes": [{ "origin": [-10, 0, 0], "size": [10, 20, 2], "uv": [22, 0] }] } ] }"#;
    let anim = r#"{ "spread": { "length": 0.5, "bones": { "wing": { "rotation": { "0": [0, 0, 0], "0.5": [0, 0, -90] } } } } }"#;
    let out = compile(&project(&[("content/wings.ymodel", model), ("content/wings.yanim", anim)]), "mymod", &yres::Refs::default()).unwrap();
    assert!(out.data_files.iter().any(|(p, _)| p == "body/models/mymod__wings.ydata"));
    let src = out.source.unwrap().all();
    assert!(src.contains("registry.register_body_model(\"mymod:wings\""), "{src}");
    assert!(src.contains("registry.register_body_anims(\"mymod:wings\""), "{src}");
    assert!(src.contains("pub const WINGS: &'static str = \"mymod:wings\";"), "{src}");
    let bad = r#"{ "texture": "@texture:minecraft:entity/bat", "bones": [{ "name": "a", "parent": "b" }] }"#;
    let e = compile(&project(&[("content/bad.ymodel", bad)]), "mymod", &yres::Refs::default()).unwrap_err();
    assert!(e.contains("must come before it"), "{e}");
}

#[test]
fn font_files_compile_validate_and_register() {
    let font = r#"{ units_per_em: 100, ascent: 80, descent: -20,
        glyphs: [{ name: "a", advance: 100, ops: [] }],
        chars: [{ char: 97, glyph: 0 }] }"#;
    let out = compile(&project(&[("content/type/a.yfont", font)]), "mymod", &yres::Refs::default()).unwrap();
    let (_, bytes) = out.data_files.iter().find(|(p, _)| p == "fonts/mymod__type__a.yfont").unwrap();
    let built = yog_font::Font::from_bytes(bytes).unwrap();
    assert_eq!(built.glyph_of('a'), Some(0));
    let src = out.source.unwrap().part("register").to_string();
    assert!(src.contains("registry.register_font_data(\"mymod:type/a\""), "{src}");
    assert!(src.contains("/generated/fonts/mymod__type__a.yfont"), "{src}");

    let invalid = r#"{ units_per_em: 100, ascent: 80, descent: -20,
        glyphs: [{ advance: 100, ops: [] }],
        chars: [{ char: 97, glyph: 4 }] }"#;
    let error = compile(&project(&[("content/bad.yfont", invalid)]), "mymod", &yres::Refs::default()).unwrap_err();
    assert!(error.contains("chars[0].glyph 4 does not name a glyph"), "{error}");
}

#[test]
fn entities_compile_register_and_name_their_models() {
    let model = r#"{ "texture": "@texture:minecraft:entity/allay/allay", "bones": [{ "name": "body", "cubes": [{ "origin": [-2, 0, -2], "size": [4, 4, 4] }] }] }"#;
    let wisp = r#"{ "name": "Wisp", "kind": "mob", "width": 0.5, "height": 0.5, "model": "wisp", "goals": ["float", "wander"],
        "attributes": { "max_health": 6, "movement_speed": 0.3 } }"#;
    let out = compile(&project(&[("content/wisp.ymodel", model), ("content/wisp.yentity", wisp)]), "mymod", &yres::Refs::default()).unwrap();
    // Each part in its file; the root only includes them.
    let src = out.source.as_ref().unwrap();
    assert!(src.part("register").contains("registry.register_entity_data(\"mymod:wisp\""), "{src:?}");
    assert!(src.part("entities").contains("pub const WISP: &'static str = \"mymod:wisp\";"), "{src:?}");
    assert!(src.root.contains("\"/generated/content/entities.g.rs\""), "{}", src.root);
    assert!(src.root.lines().all(|l| l.starts_with("//") || l.starts_with("include!(")), "{}", src.root);
    assert!(out.data_files.iter().any(|(p, _)| p == "entities/mymod__wisp.ydata"));
    let (_, data) = out.data_files.iter().find(|(p, _)| p == "entities/mymod__wisp.ydata").unwrap();
    let def: yog_registry::EntityDef = yog_data::read_bytes(data).unwrap();
    assert_eq!((def.kind, def.model.as_deref()), (yog_registry::EntityKind::Mob, Some("mymod:wisp")));
    let missing = r#"{ "kind": "object", "width": 1, "height": 1, "model": "nope" }"#;
    let e = compile(&project(&[("content/thing.yentity", missing)]), "mymod", &yres::Refs::default()).unwrap_err();
    assert!(e.contains("model `mymod:nope`: no such file"), "{e}");
}

#[test]
fn spawn_eggs_inherit_yogs_egg_tinted_with_the_entitys_colours() {
    let wisp = r#"{ "kind": "object", "width": 0.5, "height": 0.5, "spawn_egg": [1193046, 65280] }"#;
    let out = compile(&project(&[("content/wisp.yentity", wisp)]), "mymod", &yres::Refs::default()).unwrap();
    let model = output(&out, "assets/mymod/models/item/wisp_spawn_egg.json");
    assert_eq!(model["parent"], yog_ids::yog::SPAWN_EGG_MODEL);
    let def = output(&out, "assets/mymod/items/wisp_spawn_egg.json");
    assert_eq!(def["model"]["model"], "mymod:item/wisp_spawn_egg");
    assert_eq!(def["model"]["tints"][0]["value"], (0xFF12_3456_u32 as i32));
    assert_eq!(def["model"]["tints"][1]["value"], (0xFF00_FF00_u32 as i32));
}

#[test]
fn armor_materials_get_their_worn_look_as_equipment() {
    let mythril = r#"{ kind: "armor_material", id: "mythril", durability: 30, defense: { head: 3, chest: 8, legs: 6, feet: 3 },
        enchantability: 10, equip_sound: "minecraft:item.armor.equip_iron", repair: "@item:minecraft:iron_ingot" }"#;
    let root = project(&[("content/mythril.ydef", mythril), ("assets/mymod/textures/models/armor/mythril_layer_1.png", "one")]);
    let out = compile(&root, "mymod", &yres::Refs::default()).unwrap();
    let asset = output(&out, "assets/mymod/equipment/mythril.json");
    assert_eq!(asset["layers"]["humanoid"][0]["texture"], "mymod:mythril");
    assert_eq!(asset["layers"]["humanoid_leggings"][0]["texture"], "mymod:mythril");
    let worn = out.outputs.iter().find(|(p, _)| p == "assets/mymod/textures/entity/equipment/humanoid/mythril.png");
    assert_eq!(worn.map(|(_, b)| b.as_slice()), Some(&b"one"[..]));
    assert!(out.warnings.iter().any(|w| w.contains("mythril_layer_2")), "{:?}", out.warnings);
}
