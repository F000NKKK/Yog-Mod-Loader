//! Blocks drawn in 3D as the game's models make them.

use super::*;

fn solid(w: u32, h: u32) -> Image {
    Image {
        width: w,
        height: h,
        rgba: (0..w * h).flat_map(|i| [i as u8, 0, 0, 255]).collect(),
    }
}

fn opaque(shape: Shape) -> usize {
    let res = Resources {
        mod_root: None,
        vanilla: None,
    };
    let mut c = Canvas::new(32, 32, 1, &res);
    draw_with(
        &mut c,
        &[([0, 0, 0], "x".into())],
        yog_gfx::Rect::new(0.0, 0.0, 32.0, 32.0),
        (225.0, 30.0),
        |_, _| shape.clone(),
    );
    c.rgba
        .as_chunks::<{ yog_graphics::BYTES_PER_PIXEL }>()
        .0
        .iter()
        .filter(|p| p[3] > 0)
        .count()
}

#[test]
fn a_models_boxes_are_drawn_their_size_with_their_uv() {
    let img = Arc::new(solid(16, 16));
    let elements: Vec<Value> = yog_data::read_value(&yog_data::text::parse_value(
        r##"[{ "from": [5, 5, 5], "to": [11, 11, 11], "faces": {
            "up": { "uv": [5, 5, 11, 11], "texture": "#all" }, "north": { "texture": "#all" } } }]"##,
    ).unwrap())
    .unwrap();
    let shape = shape_of(&elements, &mut |v: &str| (v == "#all").then(|| img.clone()));
    assert_eq!(shape.len(), 1);
    let up = shape[0].faces[0].as_ref().unwrap();
    assert_eq!((up.width, up.height), (6, 6), "the uv's part");
    assert_eq!(up.pixel(0, 0), img.pixel(5, 5));
    let north = shape[0].faces[2].as_ref().unwrap();
    assert_eq!(
        (north.width, north.height),
        (6, 6),
        "no uv: the box's own part"
    );
    assert!(
        shape[0].faces[1].is_none(),
        "a face the model leaves out isn't drawn"
    );

    let small = opaque(shape);
    let full = opaque(cube(std::array::from_fn(|_| Some(img.clone()))));
    assert!(
        small > 0 && small * 4 < full,
        "a cable's core, not a whole block: {small} vs {full}"
    );
}

#[test]
fn parses_block_lines() {
    let b = parse("0 0 0 minecraft:stone\n1 2 -1 minecraft:furnace[facing=north]\nbad");
    assert_eq!(
        b,
        [
            ([0, 0, 0], "minecraft:stone".to_string()),
            ([1, 2, -1], "minecraft:furnace[facing=north]".to_string())
        ]
    );
}

#[test]
fn a_state_meets_multipart_conditions() {
    let (id, props) = split_state("minecraft:brewing_stand[has_bottle_0=true,has_bottle_1=false]");
    assert_eq!(id, "minecraft:brewing_stand");
    let when = |j: &str| yog_data::text::parse_value(j).unwrap();
    assert!(meets(&when(r#"{ "has_bottle_0": "true" }"#), &props));
    assert!(!meets(&when(r#"{ "has_bottle_1": "true" }"#), &props));
    assert!(meets(&when(r#"{ "has_bottle_1": "true|false" }"#), &props));
    assert!(meets(
        &when(r#"{ "OR": [{ "has_bottle_2": "true" }, { "has_bottle_0": "true" }] }"#),
        &props
    ));
    assert!(
        !meets(&when(r#"{ "has_bottle_2": "false" }"#), &props),
        "a property the state doesn't name"
    );
}

#[test]
fn turns_turn_about_their_origin() {
    let close = |a: [f32; 3], b: [f32; 3]| (0..3).all(|i| (a[i] - b[i]).abs() < 1e-4);
    // A quarter turn about y through the centre: east goes north.
    let y = Turn {
        origin: CENTRE,
        axis: 1,
        degrees: 90.0,
        rescale: false,
    };
    assert!(
        close(y.apply([16.0, 8.0, 8.0]), [8.0, 8.0, 0.0]),
        "{:?}",
        y.apply([16.0, 8.0, 8.0])
    );
    assert!(close(y.apply(CENTRE), CENTRE));
    // Rescaled at 45°: the diagonal is stretched back to the block.
    let r = Turn {
        origin: CENTRE,
        axis: 1,
        degrees: 45.0,
        rescale: true,
    };
    let d = r.apply([16.0, 8.0, 8.0]);
    assert!(((d[0] - 8.0).powi(2) + (d[2] - 8.0).powi(2)).sqrt() > 8.0 * 1.4);
}
