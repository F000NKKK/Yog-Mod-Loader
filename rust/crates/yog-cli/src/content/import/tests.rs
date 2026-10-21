use std::path::{Path, PathBuf};

use serde_json::Value;

use super::bbmodel::{base64_decode, import_bbmodel};
use super::blocks::import_pack;
use super::pack::Pack;
use super::target::Target;

fn temp(name: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("yog-import-test-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

fn target(root: &Path) -> Target {
    Target {
        root: root.to_path_buf(),
        ns: Some("mymod".into()),
        force: false,
        written: Vec::new(),
        warnings: Vec::new(),
    }
}

#[test]
fn decodes_base64() {
    assert_eq!(base64_decode("aGVsbG8=").unwrap(), b"hello");
    assert_eq!(base64_decode("aGk").unwrap(), b"hi");
}

/// A 1×1 PNG.
fn picture() -> Vec<u8> {
    yog_media::image::encode_png(&yog_media::image::Image {
        width: 1,
        height: 1,
        rgba: vec![255, 0, 0, 255],
    })
    .unwrap()
}

#[test]
fn imports_a_pack_block_and_compiles_it() {
    let pack_dir = temp("pack");
    let w = |rel: &str, s: &[u8]| {
        let p = pack_dir.join(rel);
        std::fs::create_dir_all(p.parent().unwrap()).unwrap();
        std::fs::write(p, s).unwrap();
    };
    w(
        "assets/foo/blockstates/lamp.json",
        br#"{"variants":{"":[{"model":"foo:block/lamp"},{"model":"foo:block/lamp","y":90}]}}"#,
    );
    w("assets/foo/models/block/lamp_base.json", br##"{"textures":{"0":"foo:block/lamp","particle":"#0"},"render_type":"cutout",
        "elements":[{"from":[4,0,4],"to":[12,8,12],"rotation":{"angle":20,"axis":"y","origin":[8,8,8]},
        "faces":{"north":{"uv":[0,0,8,8],"texture":"#0","cullface":"north"}}}]}"##);
    w(
        "assets/foo/models/block/lamp.json",
        br#"{"parent":"foo:block/lamp_base"}"#,
    );
    w(
        "assets/foo/models/item/lamp.json",
        br#"{"parent":"foo:block/lamp"}"#,
    );
    w("assets/foo/textures/block/lamp.png", &picture());
    w(
        "assets/foo/lang/en_us.json",
        br#"{"block.foo.lamp":"Lamp"}"#,
    );
    let root = temp("mod");
    let mut t = target(&root);
    let pack = Pack::open(&pack_dir).unwrap();
    import_pack(&mut t, &pack, Some("foo:block/lamp"), false).unwrap();
    assert!(
        t.warnings.iter().any(|w| w.contains("20° → 22.5°")),
        "{:?}",
        t.warnings
    );
    let text = crate::files::read_source(root.join("content/blocks/lamp.yblock")).unwrap();
    assert!(text.contains("\"name\": \"Lamp\""), "{text}");
    assert!(text.contains("\"render_layer\": \"cutout\""), "{text}");
    assert!(
        text.contains("\"0\": \"@texture:mymod:block/lamp\""),
        "{text}"
    );
    assert!(root.join("resources/block/lamp.ytexture").is_file());
    assert!(!text.contains("item_model"), "{text}");
    let images = crate::yres::parse(
        &crate::files::read_source(root.join("resources/BlockImages.yres")).unwrap(),
    )
    .unwrap();
    assert!(
        images
            .iter()
            .any(|e| e.name == "block.lamp" && e.value == crate::yres::Value::Bytes(picture()))
    );
    let refs = crate::yres::compile(&root, "mymod").unwrap().refs;
    let c = crate::content::compile(&root, "mymod", &refs).unwrap();
    let bs = c
        .outputs
        .iter()
        .find(|(p, _)| p == "assets/mymod/blockstates/lamp.json")
        .unwrap();
    let bs: Value = serde_json::from_slice(&bs.1).unwrap();
    assert_eq!(bs["variants"][""][1]["y"], serde_json::json!(90));
    let _ = std::fs::remove_dir_all(pack_dir);
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn imports_a_bbmodel() {
    let root = temp("bb");
    let bb = root.join("thing.bbmodel");
    std::fs::write(
        &bb,
        r#"{"meta":{"model_format":"java_block"},"name":"Thing","resolution":{"width":32,"height":32},
            "textures":[{"name":"thing.png","uuid":"u1","source":"data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR4nGP4z8DwHwAFAAH/iZk9HQAAAABJRU5ErkJggg=="}],
            "elements":[{"name":"cube","from":[0,0,0],"to":[16,8,16],"origin":[8,8,8],"rotation":[0,45,0],
              "faces":{"north":{"uv":[0,0,32,16],"texture":0},"up":{"uv":[0,0,32,32],"texture":null}}}]}"#,
    )
    .unwrap();
    let mut t = target(&root);
    import_bbmodel(&mut t, &bb, None, false).unwrap();
    let text = crate::files::read_source(root.join("content/blocks/thing.yblock")).unwrap();
    assert!(text.contains("\"uv\": [0, 0, 16, 8]"), "{text}");
    assert!(!text.contains("\"up\""), "{text}");
    let images = crate::yres::parse(
        &crate::files::read_source(root.join("resources/BlockImages.yres")).unwrap(),
    )
    .unwrap();
    assert!(images.iter().any(|e| e.name == "block.thing"));
    let refs = crate::yres::compile(&root, "mymod").unwrap().refs;
    crate::content::compile(&root, "mymod", &refs).unwrap();
    let _ = std::fs::remove_dir_all(root);
}
