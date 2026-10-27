//! `.yrecipe`s: the game's kinds and the mod's own.

use super::*;

#[test]
fn compiles_recipes() {
    let root = project(&[
        (
            "content/ruby.yitem",
            r#"{ "model": { "parent": "generated", "textures": { "layer0": "@texture:minecraft:item/emerald" } } }"#,
        ),
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
    let e = bad(
        r#"{ "type": "shaped", "pattern": ["AB"], "key": { "A": "@item:minecraft:stick" }, "result": "@item:minecraft:stick" }"#,
    );
    assert!(e.contains("pattern uses 'B'"), "{e}");
    let e = bad(
        r#"{ "type": "shapeless", "ingredients": ["stick"], "result": "@item:minecraft:stick" }"#,
    );
    assert!(
        e.contains("ingredients[0]: `stick`: name it by a reference — `@item:stick`"),
        "{e}"
    );
    let e = bad(
        r##"{ "type": "shapeless", "ingredients": ["#c:sand"], "result": "@item:minecraft:glass" }"##,
    );
    assert!(
        e.contains("`#c:sand`: write a tag as `@tag:item:c:sand`"),
        "{e}"
    );
    let e = bad(
        r##"{ "type": "smelting", "ingredient": "@item:minecraft:sand", "result": "@tag:item:c:glass" }"##,
    );
    assert!(
        e.contains("result: `@tag:item:c:glass` is a reference to tag — this takes `@item:`"),
        "{e}"
    );
    let e = bad(
        r#"{ "type": "shapeless", "ingredients": ["@item:minecraft:sand"], "result": "@item:mymod:nothing" }"#,
    );
    assert!(
        e.contains("result: no item mymod:nothing in this mod"),
        "{e}"
    );
    let e = bad(
        r#"{ "type": "shapeless", "ingredients": ["@item:minecraft:nope"], "result": "@item:minecraft:glass" }"#,
    );
    assert!(
        e.contains("ingredients[0]: no item minecraft:nope in Minecraft"),
        "{e}"
    );
    let e = bad(
        r#"{ "type": "shapeless", "pattern": ["A"], "ingredients": ["@item:minecraft:sand"], "result": "@item:minecraft:glass" }"#,
    );
    assert!(e.contains("unknown field `pattern`"), "{e}");
    let e = bad(
        r#"{ "type": "smeltin", "ingredient": "@item:minecraft:sand", "result": "@item:minecraft:glass" }"#,
    );
    assert!(
        e.contains("unknown `smeltin` — did you mean `smelting`?"),
        "{e}"
    );
}

#[test]
fn what_a_version_or_loader_hasnt_got_keeps_a_recipe_out_of_it() {
    let recipe = |key: &str| {
        format!(
            r#"{{ type: "shaped", pattern: ["D"], key: {{ D: {key} }}, result: "@item:minecraft:stick" }}"#
        )
    };
    let check = |key: &str| {
        let c = compile(
            &project(&[("content/r.yrecipe", &recipe(key))]),
            "mymod",
            &yres::Refs::default(),
        );
        c.map(|c| {
            c.warnings
                .iter()
                .filter(|w| w.message.contains("left out"))
                .map(ToString::to_string)
                .collect::<Vec<_>>()
                .join("\n")
        })
    };
    let w = check(r#""@tag:item:minecraft:music_discs""#).unwrap();
    assert!(w.contains("left out on Minecraft 1.21.1, Minecraft 26.2: #minecraft:music_discs isn't there — @tag:item:yog:music_discs is on every version and loader"), "{w}");
    assert_eq!(check(r#""@tag:item:yog:music_discs""#).unwrap(), "");
    assert_eq!(
        check(r#"["@tag:item:c:iron_ingots", "@item:minecraft:iron_ingot"]"#).unwrap(),
        "",
        "an alternative that's everywhere"
    );
    let w = check(r#""@tag:item:c:iron_ingots""#).unwrap();
    assert!(
        w.contains("left out on forge 1.19.2, forge 1.20.1, neoforge 1.20.1, Minecraft 1.21.1")
            && w.contains("@tag:item:yog:ingots/iron"),
        "{w}"
    );
    let e = check(r#""@tag:item:c:nope""#).unwrap_err();
    assert!(
        e.contains("no item tag c:nope in Minecraft or its loaders"),
        "{e}"
    );
    let e = check(r#""@tag:item:yog:nope""#).unwrap_err();
    assert!(
        e.contains("no item tag yog:nope among Yog's common tags"),
        "{e}"
    );
    assert!(
        check(r#""@tag:item:othermod:anything""#).is_ok(),
        "another mod's tags aren't known here"
    );
    let block = r#"{ tags: ["yog:planks", "yog:nope"], model: { parent: "cube_all", textures: { all: "@texture:minecraft:block/stone" } } }"#;
    let e = compile(
        &project(&[("content/p.yblock", block)]),
        "mymod",
        &yres::Refs::default(),
    )
    .unwrap_err();
    assert!(
        e.contains("tags[1]: no block tag yog:nope") && !e.contains("yog:planks"),
        "{e}"
    );
}

#[test]
fn compiles_a_recipe_of_the_mods_own_kind() {
    let kind = r#"{ kind: "recipe_kind", id: "infusion", slots: { core: { required: true }, ring: { max: 2 } }, values: { time: "number" } }"#;
    let r = r##"{ type: "mymod:infusion", slots: { core: ["@item:minecraft:diamond"], ring: [["@item:minecraft:coal", "@item:minecraft:charcoal"], "@tag:item:c:dusts"] },
                 values: { time: 200 }, result: "@item:minecraft:emerald", count: 2 }"##;
    let c = compile(
        &project(&[
            ("content/infusion.ydef", kind),
            ("content/infusion.yrecipe", r),
        ]),
        "mymod",
        &yres::Refs::default(),
    )
    .unwrap();
    let src = c.source.unwrap().all();
    assert!(
        src.contains("registry.add_recipe_data(\"mymod:infusion\""),
        "{src}"
    );
    let data = &c
        .data_files
        .iter()
        .find(|(p, _)| p.contains("infusion"))
        .unwrap()
        .1;
    let recipe = yog_data::binary::decode(data).unwrap();
    assert_eq!(
        recipe.get("type").and_then(|v| v.as_str()),
        Some("mymod:infusion")
    );
    assert_eq!(
        recipe.get("values").and_then(|v| v.get("time")),
        Some(&yog_data::Value::Int(200))
    );

    // Against its kind: slots, how many, values and their types.
    let bad = r##"{ type: "mymod:infusion", slots: { ring: ["@item:minecraft:coal", "@item:minecraft:coal", "@item:minecraft:coal"], nope: ["@item:minecraft:coal"] },
                   values: { time: "soon", heat: 3 }, result: "@item:minecraft:emerald" }"##;
    let e = compile(
        &project(&[
            ("content/infusion.ydef", kind),
            ("content/bad.yrecipe", bad),
        ]),
        "mymod",
        &yres::Refs::default(),
    )
    .unwrap_err();
    for want in [
        "slots.ring: at most 2 ingredient(s), found 3",
        "slots.nope: mymod:infusion has no slot `nope` (core, ring)",
        "slots: mymod:infusion needs `core`",
        "values.time: a number",
        "values.heat: mymod:infusion has no value `heat` (time)",
    ] {
        assert!(e.contains(want), "{want}: {e}");
    }
    let undeclared =
        r#"{ "type": "mymod:infusion", "slots": {}, "result": "@item:minecraft:emerald" }"#;
    let e = compile(
        &project(&[("content/u.yrecipe", undeclared)]),
        "mymod",
        &yres::Refs::default(),
    )
    .unwrap_err();
    assert!(
        e.contains("type `mymod:infusion`: no such kind — declare it: content/infusion.ydef"),
        "{e}"
    );
    let bad = r#"{ "type": "mymod:infusion", "ingredient": "@item:minecraft:stone", "result": "@item:minecraft:emerald" }"#;
    let e = compile(
        &project(&[
            ("content/infusion.ydef", kind),
            ("content/bad.yrecipe", bad),
        ]),
        "mymod",
        &yres::Refs::default(),
    )
    .unwrap_err();
    assert!(
        e.contains("unknown field `ingredient`") && e.contains("missing `slots`"),
        "{e}"
    );
    // Another mod's kind isn't the mod's own: not a Yog recipe type.
    let other = r#"{ "type": "create:mixing", "slots": {}, "result": "@item:minecraft:emerald" }"#;
    let e = compile(
        &project(&[("content/o.yrecipe", other)]),
        "mymod",
        &yres::Refs::default(),
    )
    .unwrap_err();
    assert!(e.contains("type `create:mixing`: one of shaped"), "{e}");
}
