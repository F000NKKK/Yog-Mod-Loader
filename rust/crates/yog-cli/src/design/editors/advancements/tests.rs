//! The advancement tab drawn and laid out as the game does.

use super::*;

fn nodes(pairs: &[(&str, Option<&str>)]) -> Vec<Node> {
    pairs
        .iter()
        .map(|(id, parent)| Node {
            id: id.to_string(),
            parent: parent.map(str::to_string),
            icon: None,
            frame: "task".into(),
            title: String::new(),
            description: String::new(),
            obtained: false,
        })
        .collect()
}

fn at(l: &[(String, (f32, f32))], id: &str) -> (f32, f32) {
    l.iter().find(|(i, _)| i == id).expect(id).1
}

#[test]
fn a_parent_is_centred_on_its_children() {
    let l = layout(&nodes(&[
        ("r", None),
        ("a", Some("r")),
        ("b", Some("r")),
        ("c", Some("r")),
    ]));
    assert_eq!(at(&l, "r"), (0.0, 1.0));
    assert_eq!(
        [at(&l, "a"), at(&l, "b"), at(&l, "c")],
        [(1.0, 0.0), (1.0, 1.0), (1.0, 2.0)]
    );
}

#[test]
fn subtrees_keep_apart() {
    let l = layout(&nodes(&[
        ("r", None),
        ("a", Some("r")),
        ("a1", Some("a")),
        ("a2", Some("a")),
        ("b", Some("r")),
    ]));
    assert_eq!(at(&l, "a1"), (2.0, 0.0));
    assert_eq!(at(&l, "a2"), (2.0, 1.0));
    assert_eq!(at(&l, "a"), (1.0, 0.5));
    assert_eq!(at(&l, "b"), (1.0, 1.5));
    assert_eq!(at(&l, "r"), (0.0, 1.0));
}

#[test]
fn a_cycle_is_still_shown() {
    assert_eq!(
        layout(&nodes(&[("a", Some("b")), ("b", Some("a"))])).len(),
        2
    );
}

#[test]
fn lines_break_at_words() {
    let w = |s: &str| s.chars().count() as f32 * 6.0;
    assert_eq!(split_lines("one two three", 48.0, &w), ["one two", "three"]);
    assert_eq!(cut_to_width("abcdef", 18.0, &w), "abc");
}

/// The picture of a `data:` URL, decoded.
fn picture(url: &str) -> yog_media::image::Image {
    use base64::Engine as _;
    let png = base64::engine::general_purpose::STANDARD
        .decode(url.trim_start_matches(crate::design::drawing::raster::PNG_URL_PREFIX))
        .unwrap();
    yog_media::image::decode(&png).unwrap()
}

#[test]
fn the_layer_slid_by_a_drag_is_the_view_drawn_at_it() {
    let mut ns = nodes(&[("a:r", None)]);
    ns.extend((0..12).map(|i| nodes(&[(&format!("a:k{i}"), Some("a:r"))]).remove(0)));
    let args = |drag: [f32; 2]| Args {
        path: String::new(),
        title: String::new(),
        background: None,
        nodes: ns.clone(),
        drag,
        hover: None,
    };
    let base = render(&args([0.0, 0.0]));
    let layer = base.layer.clone().expect("a layer");
    assert!(
        layer.drag_min[1] < 0.0 && layer.drag_max[1] > 0.0,
        "{layer:?}"
    );
    let slid = picture(&layer.image);
    let window = picture(&layer.window);
    for drag in [
        [0.0, 0.0],
        [0.0, -7.5],
        [0.0, layer.drag_min[1]],
        [0.0, layer.drag_max[1]],
    ] {
        let screen = render(&args(drag));
        let shot = picture(screen.image.as_deref().unwrap());
        // As a tool slides it: the drag's whole pixels from the drawn one's.
        let dy = drag[1].floor() - base.drag[1].floor();
        let [vx, vy, vw, vh] = screen.view.map(|v| v as i64);
        for y in 0..vh {
            for x in 0..vw {
                let (lx, ly) = (x - layer.at[0] as i64, y - layer.at[1] as i64 - dy as i64);
                let px = |img: &yog_media::image::Image, x: i64, y: i64| {
                    let i = ((y * img.width as i64 + x) * 4) as usize;
                    img.rgba[i..i + 4].to_vec()
                };
                // Where the window lies over the view, it's laid over the layer too.
                if px(&window, vx + x, vy + y)[3] == 0 {
                    assert_eq!(
                        px(&shot, vx + x, vy + y),
                        px(&slid, lx, ly),
                        "drag {drag:?} at {x},{y}"
                    );
                }
            }
        }
    }
}
