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
        r#"{ item: { tool: { kind: "pickaxe", material: "iron" }, armor: { slot: "head", material: "iron" }, wearable: "head" } }"#,
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
        r#"{ item: { tool: { kind: "pickaxe", material: "iron" }, wearable: "head" } }"#,
    );
    assert!(fine.is_empty(), "{fine:?}");
}

#[test]
fn a_durable_item_stacking_is_a_warning() {
    let doc =
        yog_data::text::parse_value(r#"{ item: { durability: 50, stack_size: 16 } }"#).unwrap();
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
        r#"{ item: { hangs: { entity: "@entity:mymod:scroll", size: [0, 1] } } }"#,
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
