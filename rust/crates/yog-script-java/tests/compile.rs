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
