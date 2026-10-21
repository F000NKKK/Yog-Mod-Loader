//! A frame and ops through Yog Data and back, and ops adding up.

use super::*;

fn quad() -> Quad {
    Quad {
        pos: [Vec3::new(1.0, 2.0, 3.0); 4],
        uv: [[0.5, 0.25]; 4],
        normal: Vec3::new(0.0, 1.0, 0.0),
    }
}

#[test]
fn a_frame_reads_back() {
    let f = Frame {
        uuid: "u".into(),
        entity_type: "minecraft:player".into(),
        age: 12.5,
        pos: [1.0, 2.0, 3.0],
        flags: 5,
        game_time: 1234.5,
        data: vec![Kept {
            key: "m:k".into(),
            value: vec![1, 2],
        }],
        captures: vec![Capture {
            source: "self".into(),
            meshes: vec![Mesh::new("t", &[quad()])],
        }],
        ..Default::default()
    };
    let back: Frame = yog_data::read_bytes(&yog_data::to_bytes(&f)).unwrap();
    assert_eq!(back, f);
    assert_eq!(back.captures[0].meshes[0].quads(), vec![quad()]);
    assert_eq!(back.data("m:k"), Some(&[1u8, 2][..]));
}

#[test]
fn ops_add_up_and_turns_compose() {
    let mut a = Ops::default();
    a.hide();
    a.root(&Mat4::translation(Vec3::new(1.0, 0.0, 0.0)));
    let mut b = Ops::default();
    b.disguise("minecraft:zombie");
    b.root(&Mat4::translation(Vec3::new(0.0, 2.0, 0.0)));
    b.mesh(
        &Anchor::Bone("head".into()),
        "t",
        Look::default(),
        &[quad()],
    );
    a.add(b);
    assert!(a.hide);
    assert_eq!(a.disguise.as_deref(), Some("minecraft:zombie"));
    assert_eq!(a.draws[0].anchor, AnchorKind::Bone);
    let both =
        Mat4::translation(Vec3::new(1.0, 0.0, 0.0)) * Mat4::translation(Vec3::new(0.0, 2.0, 0.0));
    assert_eq!(a.root, Some(both.0.to_vec()));
    let back: Ops = yog_data::read_bytes(&yog_data::to_bytes(&a)).unwrap();
    assert_eq!(back, a);
}
