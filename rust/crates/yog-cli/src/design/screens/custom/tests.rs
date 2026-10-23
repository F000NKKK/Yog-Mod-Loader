use super::*;
use crate::design::screens::custom::fit::marker;
use crate::design::screens::markup::YuiNode;

fn node(uid: &str, kind: &str, attrs: &[(&str, &str)], children: Vec<YuiNode>) -> YuiNode {
    YuiNode {
        uid: uid.into(),
        kind: kind.into(),
        attrs: attrs
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect(),
        children,
    }
}

#[test]
fn custom_elements_become_marked_spacers_with_placeholders() {
    let root = node(
        "s",
        "Screen",
        &[("width", "100"), ("height", "60")],
        vec![
            node(
                "c",
                "StatBar",
                &[("x", "4"), ("y", "6"), ("w", "50"), ("value", "0.5")],
                vec![],
            ),
            node("l", "Label", &[("text", "hi")], vec![]),
        ],
    );
    assert!(is_component("StatBar") && !is_component("Label") && !is_component("statBar"));
    let p = prepare(&root, None, 200.0, 100.0);
    let c = &p.root.children[0];
    assert_eq!(c.kind, "Spacer");
    assert_eq!(c.attr("w"), Some("50"));
    assert_eq!(c.attr("h"), Some("24"));
    assert_eq!(c.attr("bg"), Some("#01D00000"));
    assert!(c.attr("value").is_none());
    assert_eq!(
        p.slots[0].req.attrs,
        vec![("value".to_string(), "0.5".to_string())]
    );

    let mut ops = vec![
        DrawOp::Rect {
            x1: 4.0,
            y1: 6.0,
            x2: 54.0,
            y2: 30.0,
            color: marker(0),
        },
        DrawOp::ClearScissor,
    ];
    let rects = HashMap::from([("c".to_string(), [4.0, 6.0, 50.0, 24.0])]);
    splice(&mut ops, &p, &rects);
    assert!(matches!(
        ops[0],
        DrawOp::Scissor {
            x: 4,
            y: 6,
            w: 50,
            h: 24
        }
    ));
    assert!(
        ops.iter()
            .any(|o| matches!(o, DrawOp::Text { text, x, .. } if text == "<StatBar>" && *x == 7.0))
    );
}

/// The whole path — `yog designer-lib`, the host process, splicing — on a real mod:
/// `YOG_DESIGNER_MOD=../../Yog-Modlist YOG_IDLE_DESIGNER_HOST=target/debug/yog-idle-tauri
/// YOG_DESIGNER_PNG=out.png cargo test --lib designer::custom -- --ignored`
#[test]
#[ignore]
fn draws_a_mods_components() {
    let (Ok(root), Ok(out)) = (
        std::env::var(yog_ids::env::DESIGNER_MOD),
        std::env::var(yog_ids::env::DESIGNER_PNG),
    ) else {
        return;
    };
    let root = std::fs::canonicalize(root).unwrap();
    let started = Instant::now();
    while status(&root).status == Status::Building || started.elapsed() < DEBOUNCE {
        assert!(
            started.elapsed() < Duration::from_secs(600),
            "build took too long"
        );
        std::thread::sleep(Duration::from_millis(200));
    }
    let snap = status(&root);
    assert_eq!(snap.status, Status::Ready, "{:?}", snap.error);
    eprintln!(
        "{}: {:?}",
        snap.mod_name,
        snap.components.iter().map(|c| &c.name).collect::<Vec<_>>()
    );
    let doc = node(
        "s",
        "Screen",
        &[("width", "160"), ("height", "70"), ("bg", "#E0101010")],
        vec![
            node(
                "p",
                "Pill",
                &[("x", "6"), ("y", "6"), ("label", "Custom!")],
                vec![],
            ),
            node(
                "b",
                "StatBar",
                &[
                    ("x", "6"),
                    ("y", "24"),
                    ("w", "140"),
                    ("label", "Loaded"),
                    ("value", "0.7"),
                ],
                vec![],
            ),
            node(
                "m",
                "Missing",
                &[("x", "6"), ("y", "44"), ("w", "140"), ("h", "22")],
                vec![],
            ),
        ],
    );
    let args = crate::design::RenderArgs {
        path: root.join("src/demo.yui").to_string_lossy().into_owned(),
        root: doc,
        screen_w: 170,
        screen_h: 80,
        scale: 4,
        preview: false,
    };
    let r = crate::design::render(&args).unwrap();
    use base64::Engine as _;
    let png = base64::engine::general_purpose::STANDARD
        .decode(r.image.trim_start_matches("data:image/png;base64,"))
        .unwrap();
    std::fs::write(out, png).unwrap();
    let pill = r.rects.iter().find(|n| n.uid == "p").unwrap();
    assert!(
        pill.rect[2] > 10.0 && pill.rect[3] < 20.0,
        "{:?}",
        pill.rect
    );
}

#[test]
fn cargo_errors_are_picked_out() {
    let e = build_errors(
        "   Compiling x\nerror[E0425]: cannot find value `y`\n  --> src/lib.rs:3:5\n   |\nerror: could not compile `x`\n",
    );
    assert_eq!(e, "error[E0425]: cannot find value `y`\n--> src/lib.rs:3:5");
}
