//! Content declarations made definitions: their kind's defaults, their
//! properties over them, checked by the kind's schema; registered when the
//! mods load.

use yog_data::Value;

/// What a test's source is compiled for: package `p`, mod `test`.
fn target(defs: &[yog_script_java::ContentDef]) -> yog_script_java::Target<'_> {
    yog_script_java::Target {
        package: "p",
        mod_id: Some("test"),
        defs,
    }
}

const SRC: &str = "block Lamp {\n    Hardness = 2.0;\n    LightLevel = 15;\n}\nitem Gem {\n    MaxStack = 16;\n}\n";

#[test]
fn content_is_its_kinds_definition() {
    let checked = yog_script_analyze::check("a.ys", SRC);
    assert!(checked.problems.is_empty(), "{:#?}", checked.problems);
    let defs = yog_script_build::content_defs(("a.ys", SRC), &checked, "test")
        .unwrap_or_else(|p| panic!("{p:#?}"));
    let ids: Vec<_> = defs
        .iter()
        .map(|d| (d.kind.as_str(), d.id.as_str()))
        .collect();
    assert_eq!(ids, [("block", "test:lamp"), ("item", "test:gem")]);
    let lamp = yog_data::binary::decode(&defs[0].data).unwrap_or_default();
    let Value::Map(fields) = lamp else {
        panic!("a definition is a map");
    };
    assert_eq!(fields.get("hardness"), Some(&Value::Float(2.0)));
    assert_eq!(fields.get("light_level"), Some(&Value::Int(15)));
    // The rest its defaults: stone's resistance.
    assert!(matches!(fields.get("resistance"), Some(Value::Float(_))));
    let java = yog_script_java::compile_in(("a.ys", SRC), &checked, &target(&defs))
        .unwrap_or_else(|p| panic!("{p:#?}"));
    assert!(
        java.source.contains(
            "new dev.yog.bus.YogServices.Content.Register(\"block\", \"test:lamp\", new byte[] {"
        ),
        "{}",
        java.source
    );
    assert!(
        java.mods.iter().any(|m| m == "p.AScript$$Load"),
        "{:?}",
        java.mods
    );
}

#[test]
fn a_definition_its_kind_refuses_is_reported() {
    // A light level past what its field holds (a byte).
    let src = "block Lamp {\n    LightLevel = 300;\n}\n";
    let checked = yog_script_analyze::check("a.ys", src);
    let defs = yog_script_build::content_defs(("a.ys", src), &checked, "test");
    assert!(defs.is_err());
}

/// Content with members is its kind's object's class, its objects made by
/// it once it's registered when the mods load.
#[test]
fn content_with_members_is_its_kinds_object() {
    let src = "block Lamp {\n    LightLevel = 15;\n    public i32 Uses = 0;\n    Lamp() {\n        Use += player => {\n            Uses += 1;\n        };\n    }\n    public i32 Height() => Pos.Y;\n}\n";
    let checked = yog_script_analyze::check("a.ys", src);
    assert!(checked.problems.is_empty(), "{:#?}", checked.problems);
    let defs = yog_script_build::content_defs(("a.ys", src), &checked, "test")
        .unwrap_or_else(|p| panic!("{p:#?}"));
    let java = yog_script_java::compile_in(("a.ys", src), &checked, &target(&defs))
        .unwrap_or_else(|p| panic!("{p:#?}"))
        .source;
    for want in [
        "public static class Lamp extends dev.yog.script.yog.BlockObject {",
        "Use.add((player) -> {",
        "return Pos().y();",
        "dev.yog.script.yog.BlockObject.register(\"test:lamp\", Lamp::new);",
    ] {
        assert!(java.contains(want), "missing `{want}` in:\n{java}");
    }
}
