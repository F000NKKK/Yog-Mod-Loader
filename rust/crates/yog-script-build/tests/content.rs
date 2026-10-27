//! Content declarations are their files' behaviour: what a block is is in
//! its `.yblock` (the generated Rust registers it), the declaration's
//! class makes its objects for the content of its id.

/// What a test's source is compiled for: package `p`, mod `test`.
const TARGET: yog_script_java::Target<'static> = yog_script_java::Target {
    package: "p",
    mod_id: Some("test"),
    others: &[],
};

/// Content with members is its kind's object's class, its objects made by
/// it for the content of its id (`lamp.yblock`'s); nothing registers it here.
#[test]
fn content_with_members_is_its_kinds_object() {
    let src = "block Lamp {\n    public i32 Uses = 0;\n    Lamp() {\n        Use += player => {\n            Uses += 1;\n        };\n    }\n    public i32 Height() => Pos.Y;\n}\n";
    let checked = yog_script_analyze::check("A.ys", src);
    assert!(checked.problems.is_empty(), "{:#?}", checked.problems);
    let java = yog_script_java::compile_in(("A.ys", src), &checked, &TARGET)
        .unwrap_or_else(|p| panic!("{p:#?}"));
    for want in [
        "public static class Lamp extends dev.yog.script.yog.BlockObject {",
        "Use.add((player) -> {",
        "return Pos().y();",
        "dev.yog.script.yog.BlockObject.register(\"test:lamp\", Lamp::new);",
    ] {
        assert!(
            java.source.contains(want),
            "missing `{want}` in:\n{}",
            java.source
        );
    }
    assert!(
        !java.source.contains("Content.Register"),
        "its file registers it: {}",
        java.source
    );
    assert!(
        java.mods.iter().any(|m| m == "p.AScript$$Load"),
        "{:?}",
        java.mods
    );
}

/// A declaration of a multi-word name binds to its file's snake-case one.
#[test]
fn content_binds_by_its_files_name() {
    let src = "item RubyGem {\n    RubyGem() {\n        Use += player => { };\n    }\n}\n";
    let checked = yog_script_analyze::check("A.ys", src);
    assert!(checked.problems.is_empty(), "{:#?}", checked.problems);
    let java = yog_script_java::compile_in(("A.ys", src), &checked, &TARGET)
        .unwrap_or_else(|p| panic!("{p:#?}"))
        .source;
    assert!(
        java.contains("dev.yog.script.yog.ItemObject.register(\"test:ruby_gem\", RubyGem::new);"),
        "{java}"
    );
}

/// A block's `saved` members: its object keeps them as `State.Saved` (a
/// record a member, keyed `Block.Member`), BlockObject reading them in
/// from its chunk's store as it makes the object.
#[test]
fn saved_block_members() {
    let src = "block Lamp {\n    public saved i32 Uses = 0;\n    Lamp() {\n        Use += player => {\n            Uses += 1;\n        };\n    }\n}\n";
    let checked = yog_script_analyze::check("A.ys", src);
    assert!(checked.problems.is_empty(), "{:#?}", checked.problems);
    let java = yog_script_java::compile_in(("A.ys", src), &checked, &TARGET)
        .unwrap_or_else(|p| panic!("{p:#?}"));
    for want in [
        "public static class Lamp extends dev.yog.script.yog.BlockObject implements dev.yog.script.yog.State.Saved {",
        "public record $Uses(int value) {}",
        "read.apply(\"Lamp.Uses\")",
        "write.accept(\"Lamp.Uses\", dev.yog.api.YogBus.bytes(new $Uses(Uses)));",
    ] {
        assert!(java.source.contains(want), "missing `{want}` in:\n{}", java.source);
    }
}
