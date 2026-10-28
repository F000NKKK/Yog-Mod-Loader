//! The rules as an editor's live check finds them: `problems` of a file's text.

use yog_ids::Format;

/// The messages of what `problems` finds in `text`, a file of `format`.
fn found(format: Format, text: &str) -> Vec<String> {
    let doc = yog_data::text::parse_value(text).unwrap_or_else(|e| panic!("{e}"));
    crate::problems(format, &doc)
        .into_iter()
        .map(|d| d.message)
        .collect()
}

#[test]
fn an_item_is_a_tool_or_armor_and_armor_is_worn_already() {
    let both = found(
        Format::Item,
        r#"{ model: { parent: "generated", textures: { layer0: "@texture:mymod:item/x" } }, item: { tool: { kind: "pickaxe", material: "iron" }, armor: { slot: "head", material: "iron" }, wearable: "head" } }"#,
    );
    assert!(
        both.iter().any(|m| m.contains("a tool or armor, not both")),
        "{both:?}"
    );
    assert!(
        both.iter().any(|m| m.contains("armor is worn already")),
        "{both:?}"
    );
    let fine = found(
        Format::Item,
        r#"{ model: { parent: "generated", textures: { layer0: "@texture:mymod:item/x" } }, item: { tool: { kind: "pickaxe", material: "iron" }, wearable: "head" } }"#,
    );
    assert!(fine.is_empty(), "{fine:?}");
}

#[test]
fn a_durable_item_stacking_is_a_warning() {
    let doc =
        yog_data::text::parse_value(r#"{ model: { parent: "generated", textures: { layer0: "@texture:mymod:item/x" } }, item: { durability: 50, stack_size: 16 } }"#).unwrap();
    let found = crate::problems(Format::Item, &doc);
    assert_eq!(found.len(), 1, "{found:?}");
    assert!(!found[0].is_error());
}

#[test]
fn a_template_gives_the_shape_so_not_elements_too() {
    let element =
        r##"[{ from: [0, 0, 0], to: [16, 16, 16], faces: { up: { texture: "#all" } } }]"##;
    let both = found(
        Format::Block,
        &format!(
            r#"{{ model: {{ parent: "cube_all", textures: {{ all: "@texture:mymod:block/ruby" }}, elements: {element} }} }}"#
        ),
    );
    assert!(
        both.iter()
            .any(|m| m.contains("elements replace the shape of parent \"cube_all\"")),
        "{both:?}"
    );
    // A parent model by id takes elements: they replace its shape.
    let raw = found(
        Format::Block,
        &format!(
            r#"{{ model: {{ parent: "minecraft:block/block", textures: {{ all: "@texture:mymod:block/ruby" }}, elements: {element} }} }}"#
        ),
    );
    assert!(raw.is_empty(), "{raw:?}");
    let state = found(
        Format::Block,
        &format!(
            r#"{{ model: {{ parent: "cube_all", textures: {{ all: "@texture:mymod:block/ruby" }} }}, states: {{ models: {{ lit: {{ parent: "cube_all", elements: {element} }} }} }} }}"#
        ),
    );
    assert!(
        state.iter().any(|m| m.contains("elements replace")),
        "a state's model too: {state:?}"
    );
}

#[test]
fn a_hanging_thing_is_at_least_a_block() {
    let flat = found(
        Format::Item,
        r#"{ model: { parent: "generated", textures: { layer0: "@texture:mymod:item/x" } }, item: { hangs: { entity: "@entity:mymod:scroll", size: [0, 1] } } }"#,
    );
    assert!(
        flat.iter()
            .any(|m| m.contains("at least a block wide and high")),
        "{flat:?}"
    );
}

/// A block of `states`, its model a plain cube.
fn block_states(states: &str) -> Vec<String> {
    found(
        Format::Block,
        &format!(
            r#"{{ model: {{ parent: "cube_all", textures: {{ all: "@texture:mymod:block/ruby" }} }}, states: {states} }}"#
        ),
    )
}

#[test]
fn a_blocks_states_go_together() {
    assert!(
        block_states(r#"{ rotations: [{ y: 45 }] }"#)
            .iter()
            .any(|m| m == "must be 0, 90, 180 or 270")
    );
    assert!(
        block_states(r#"{ rotations: [] }"#)
            .iter()
            .any(|m| m.contains("a non-empty list"))
    );
    let mixed = block_states(r#"{ facing: "horizontal", rotations: [{ y: 90 }] }"#);
    assert!(
        mixed
            .iter()
            .any(|m| m.contains("can't go with rotations or connect")),
        "{mixed:?}"
    );
    let attached = block_states(r#"{ attach: true, facing: "all" }"#);
    assert!(
        attached.iter().any(|m| m.contains("turns four ways")),
        "{attached:?}"
    );
    let lone = block_states(r#"{ placement: "toward" }"#);
    assert!(
        lone.iter()
            .any(|m| m.contains("need facing, attach, waterlogged or properties")),
        "{lone:?}"
    );
    assert!(block_states(r#"{ facing: "horizontal", placement: "toward" }"#).is_empty());
}

#[test]
fn a_blocks_own_properties_are_named_and_have_two_values() {
    let bad = block_states(r#"{ properties: { Lit: ["on", "on"], facing: ["a", "b"] } }"#);
    assert!(
        bad.iter()
            .any(|m| m.contains("lowercase letters, digits and _") && m.contains("Lit")),
        "{bad:?}"
    );
    assert!(
        bad.iter()
            .any(|m| m.contains("comes from facing, attach or waterlogged")),
        "{bad:?}"
    );
    assert!(
        bad.iter()
            .any(|m| m.contains("two or more different values")),
        "{bad:?}"
    );
    assert!(block_states(r#"{ properties: { lit: ["off", "on"] } }"#).is_empty());
}

/// A block of `model`.
fn block_model(model: &str) -> Vec<String> {
    found(Format::Block, &format!("{{ model: {model} }}"))
}

#[test]
fn a_models_parent_is_a_template_of_its_kind_or_a_model_id() {
    assert!(
        block_model(r#"{ parent: "generated", textures: { layer0: "@texture:m:item/x" } }"#)
            .iter()
            .any(|m| m.contains("is an item sprite"))
    );
    assert!(
        block_model(r#"{ parent: "cubeall" }"#)
            .iter()
            .any(|m| m.contains("isn't a template"))
    );
    assert!(block_model(r#"{ parent: "minecraft:block/stairs" }"#).is_empty());
    assert!(
        block_model("{}")
            .iter()
            .any(|m| m.contains("no parent and no elements"))
    );
}

#[test]
fn a_template_needs_its_slots_unless_a_material_gives_them() {
    let missing =
        block_model(r#"{ parent: "cube_column", textures: { end: "@texture:m:block/end" } }"#);
    assert_eq!(missing, ["parent \"cube_column\" needs textures.side"]);
    assert!(
        block_model(r#"{ parent: "cube_column", material: "@material:m:block/log" }"#).is_empty()
    );
    let sprite = found(
        Format::Item,
        r#"{ model: { parent: "generated", textures: { layer0: "@texture:m:item/a", layer2: "@texture:m:item/c", glow: "@texture:m:item/g" } } }"#,
    );
    assert!(
        sprite.iter().any(|m| m == "without the layer before it"),
        "{sprite:?}"
    );
    assert!(
        sprite
            .iter()
            .any(|m| m.contains("isn't used by an item sprite")),
        "{sprite:?}"
    );
}

#[test]
fn an_elements_faces_name_the_models_slots_and_turn_at_right_angles() {
    let own = |faces: &str, rotation: &str| {
        block_model(&format!(
            r#"{{ textures: {{ all: "@texture:m:block/x" }}, elements: [{{ from: [0, 0, 0], to: [16, 16, 16], {rotation} faces: {faces} }}] }}"#
        ))
    };
    assert!(own(r##"{ up: { texture: "#all" } }"##, "").is_empty());
    assert!(
        own(r##"{ up: { texture: "#side" } }"##, "")
            .iter()
            .any(|m| m.contains("no such slot in textures"))
    );
    assert!(
        own(r##"{ up: { texture: "#all", rotation: 45 } }"##, "")
            .iter()
            .any(|m| m == "must be 0, 90, 180 or 270")
    );
    assert!(
        own(
            r##"{ up: { texture: "#all" } }"##,
            r#"rotation: { axis: "y", angle: 30 },"#
        )
        .iter()
        .any(|m| m.contains("Minecraft rotates elements by"))
    );
}

#[test]
fn a_kinds_block_gives_only_the_kinds_textures() {
    let kind = |model: &str| {
        found(
            Format::Block,
            &format!(r#"{{ kind: "stairs", model: {model} }}"#),
        )
    };
    assert!(
        kind(r#"{ parent: "cube_all" }"#)
            .iter()
            .any(|m| m.contains("give only textures"))
    );
    assert!(
        kind(r#"{ textures: { side: "@texture:m:block/s" } }"#)
            .iter()
            .any(|m| m.contains("needs textures.top"))
    );
}

/// A cube block with `block` and `kind` (or none).
fn block_props(kind: &str, block: &str) -> Vec<String> {
    let kind = if kind.is_empty() {
        String::new()
    } else {
        format!(r#"kind: "{kind}", "#)
    };
    let model = if kind.is_empty() {
        r#"{ parent: "cube_all", textures: { all: "@texture:m:block/x" } }"#
    } else {
        r#"{ textures: { bottom: "@texture:m:block/x", top: "@texture:m:block/x", side: "@texture:m:block/x" } }"#
    };
    found(
        Format::Block,
        &format!("{{ {kind}model: {model}, block: {block} }}"),
    )
}

#[test]
fn a_blocks_fields_go_with_its_kind() {
    assert!(
        block_props("", r#"{ tool_level: "iron" }"#)
            .iter()
            .any(|m| m.contains("tool_level without tool"))
    );
    assert!(
        block_props("", r#"{ inventory: "chest" }"#)
            .iter()
            .any(|m| m.contains("the id of an inventory screen"))
    );
    assert!(
        block_props("", r#"{ base: "@block:m:planks" }"#)
            .iter()
            .any(|m| m.contains("only a block of a kind takes it"))
    );
    let stairs = block_props("stairs", r#"{ redstone: true }"#);
    assert!(
        stairs
            .iter()
            .any(|m| m.contains("redstone doesn't go with kind stairs")),
        "{stairs:?}"
    );
    assert!(
        stairs.iter().any(|m| m.contains("is made from a block")),
        "{stairs:?}"
    );
    assert!(block_props("stairs", r#"{ base: "@block:m:planks" }"#).is_empty());
}
