//! A mod's sources are one project, as a C# project's files: each reaches
//! the others' types, in Java nested in their files' classes.

use yog_script_analyze::ast::Decl;
use yog_script_analyze::{Env, check_in, declarations};

const SHAPES: &str = "public record Size(i32 W, i32 H) {\n    public i32 Area() => W * H;\n}\n";
const MAIN: &str = "i32 Floor() {\n    let s = new Size(3, 4);\n    return s.Area();\n}\n";

#[test]
fn a_source_reaches_another_sources_types() {
    let shapes: Vec<Decl> = declarations("shapes.ys", SHAPES, Env::default());
    let others: [&[Decl]; 1] = [&shapes];
    let env = Env {
        others: &others,
        ..Env::default()
    };
    let checked = check_in("main.ys", MAIN, env);
    assert!(checked.problems.is_empty(), "{:#?}", checked.problems);
    let named: [(&str, &[Decl]); 1] = [("shapes.ys", &shapes)];
    let target = yog_script_java::Target {
        package: "p",
        mod_id: Some("test"),
        others: &named,
    };
    let java = yog_script_java::compile_in(("main.ys", MAIN), &checked, &target)
        .unwrap_or_else(|p| panic!("{p:#?}"))
        .source;
    assert!(java.contains("new p.ShapesScript.Size(3, 4)"), "{java}");
}

#[test]
fn another_sources_own_methods_stay_its_own() {
    let helpers = declarations("helpers.ys", "i32 Twice(i32 x) => x * 2;\n", Env::default());
    let others: [&[Decl]; 1] = [&helpers];
    let env = Env {
        others: &others,
        ..Env::default()
    };
    let checked = check_in("main.ys", "i32 Four() => Twice(2);\n", env);
    assert!(
        !checked.problems.is_empty(),
        "a file's methods aren't another's to call by name"
    );
}

/// The content's generated source (`yog-content.g.ys`): its ids by name,
/// reached from the mod's own code.
#[test]
fn the_contents_ids_are_reached_by_name() {
    let generated =
        "public static class Blocks {\n    public const string Lamp = \"test:lamp\";\n}\n";
    let name = "obj/generated/yog-content.g.ys";
    let content = declarations(name, generated, Env::default());
    let others: [&[Decl]; 1] = [&content];
    let env = Env {
        others: &others,
        ..Env::default()
    };
    let main = "string LampId() => Blocks.Lamp;\n";
    let checked = check_in("main.ys", main, env);
    assert!(checked.problems.is_empty(), "{:#?}", checked.problems);
    let named: [(&str, &[Decl]); 1] = [(name, &content)];
    let target = yog_script_java::Target {
        package: "p",
        mod_id: Some("test"),
        others: &named,
    };
    let java = yog_script_java::compile_in(("main.ys", main), &checked, &target)
        .unwrap_or_else(|p| panic!("{p:#?}"))
        .source;
    assert!(java.contains(".Blocks.Lamp"), "{java}");
}
