//! `.yitem`s: tools, armor, potions, resource texts.

use super::*;

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
    assert!(src.contains(r#".tool("pickaxe", "mymod:mythril")"#) && !src.contains(".tool_attack("), "the game's tool's attack: {src}");
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
