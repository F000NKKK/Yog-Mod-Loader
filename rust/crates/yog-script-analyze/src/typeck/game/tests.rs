use super::*;
use crate::index::{ClassIndex, Side};

fn index() -> ClassIndex {
    let m = |name: &str, desc: &str| Member {
        name: name.into(),
        desc: desc.into(),
        access: access::PUBLIC,
    };
    ClassIndex {
        mc: "1.21.1".into(),
        classes: vec![
            Class {
                name: "net/minecraft/world/entity/LivingEntity".into(),
                side: Side::Shared,
                access: access::PUBLIC,
                extends: None,
                implements: vec![],
                fields: vec![m("noActionTime", "I")],
                methods: vec![
                    m("getHealth", "()F"),
                    m(
                        "hurt",
                        "(Lnet/minecraft/world/damagesource/DamageSource;F)Z",
                    ),
                    m("isAlive", "()Z"),
                ],
            },
            Class {
                name: "net/minecraft/world/entity/player/Player".into(),
                side: Side::Shared,
                access: access::PUBLIC,
                extends: Some("net/minecraft/world/entity/LivingEntity".into()),
                implements: vec![],
                fields: vec![],
                methods: vec![m("getName", "()Ljava/lang/String;")],
            },
        ],
    }
}

#[test]
fn descriptors_are_types() {
    assert_eq!(parse_desc("I").map(|(t, _)| t), Some(Type::I32));
    assert_eq!(
        parse_desc("Ljava/lang/String;").map(|(t, _)| t),
        Some(Type::String)
    );
    assert_eq!(
        parse_desc("[F").map(|(t, _)| t),
        Some(Type::List(Box::new(Type::F32)))
    );
    let (params, ret) = method_types("(Lnet/minecraft/A;F)Z").unwrap();
    assert_eq!(params, [Type::Game("net/minecraft/A".into()), Type::F32]);
    assert_eq!(ret, Type::Bool);
}

#[test]
fn members_are_found_by_csharp_names_through_the_hierarchy() {
    let index = index();
    let lookup = index.lookup();
    let player = lookup
        .class("net/minecraft/world/entity/player/Player")
        .unwrap();
    assert!(matches!(
        member(&lookup, player, "Health", false),
        Found::Value(Type::F32)
    ));
    assert!(matches!(
        member(&lookup, player, "IsAlive", false),
        Found::Value(Type::Bool)
    ));
    assert!(matches!(
        member(&lookup, player, "Name", false),
        Found::Value(Type::String)
    ));
    assert!(matches!(
        member(&lookup, player, "NoActionTime", false),
        Found::Value(Type::I32)
    ));
    assert!(matches!(member(&lookup, player, "Hurt", true), Found::Methods(ref m) if m.len() == 1));
    assert!(matches!(
        member(&lookup, player, "Fly", true),
        Found::Nothing
    ));
}
