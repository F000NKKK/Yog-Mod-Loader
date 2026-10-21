//! Bodies, fonts and entities.

use super::*;

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
