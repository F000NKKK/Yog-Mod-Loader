//! Sources compiled to Java: what's written, and what isn't yet.

use yog_ids::script_problem;
use yog_script_java::compile;

fn java(src: &str) -> Result<String, Vec<String>> {
    let checked = yog_script_analyze::check("sample.ys", src);
    assert!(checked.problems.is_empty(), "{:#?}", checked.problems);
    compile("sample.ys", src, &checked, "yogscript.test")
        .map(|c| c.source)
        .map_err(|p| {
            p.into_iter()
                .map(|d| format!("{:?} {}", d.code, d.message))
                .collect()
        })
}

#[test]
fn a_program_compiles() {
    let out = java(include_str!("sample.ys")).unwrap_or_else(|p| panic!("{p:#?}"));
    for want in [
        "package yogscript.test;",
        "public final class SampleScript {",
        "public sealed interface Mode permits Mode.Idle, Mode.Charging, Mode.Full {",
        "record Charging(int Rate) implements Mode {}",
        "public enum Tier {",
        "public static abstract class Machine {",
        "public static class Battery extends Machine {",
        "public record Pos(int X, int Y) {",
        "dev.yog.script.Result<Integer, dev.yog.script.Error>",
    ] {
        assert!(out.contains(want), "missing `{want}` in:\n{out}");
    }
}

#[test]
fn what_isnt_written_yet_is_said() {
    let src = "void F(ref i32 x) { }\n";
    let checked = yog_script_analyze::check("a.ys", src);
    let err = compile("a.ys", src, &checked, "p")
        .err()
        .unwrap_or_default();
    assert_eq!(
        err.iter()
            .filter_map(|d| d.code.as_deref())
            .collect::<Vec<_>>(),
        [script_problem::NOT_COMPILED]
    );
}

#[test]
fn yogs_surface_is_the_bus() {
    let src = include_str!("../../yog-script-analyze/tests/fixtures/glassy.ys");
    let checked = yog_script_analyze::check("glassy.ys", src);
    let out = compile("glassy.ys", src, &checked, "yogscript.glassy")
        .map(|c| c.source)
        .unwrap_or_else(|p| panic!("{p:#?}"));
    for want in [
        "public static final String Prefix = \"g\";",
        "dev.yog.bus.YogEvents.Block.Break.on(0, (e) -> {",
        "if (e.playerName().startsWith(Prefix)) {",
        "new dev.yog.bus.YogServices.World.SetBlock(e.dimension(), e.pos(), \"minecraft:glass\").call();",
        "return null;",
    ] {
        assert!(out.contains(want), "missing `{want}` in:\n{out}");
    }
}

#[test]
fn an_indexer_is_its_accessors() {
    let src = "public class Grid {\n    List<i32> _cells = [0, 0];\n    public i32 this[i32 i] {\n        get => _cells[i];\n        set { _cells[i] = value; }\n    }\n}\n\
               void F(Grid g) {\n    g[0] = g[1];\n    g[1] += 2;\n}\n";
    let out = java(src).unwrap_or_else(|p| panic!("{p:#?}"));
    for want in [
        "public int get_Item(int i)",
        "public void set_Item(int i, int value)",
        "g.set_Item(0, g.get_Item(1));",
        "g.set_Item(1, g.get_Item(1) + 2);",
    ] {
        assert!(out.contains(want), "missing `{want}` in:\n{out}");
    }
}
