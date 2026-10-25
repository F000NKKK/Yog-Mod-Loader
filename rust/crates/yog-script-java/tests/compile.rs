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

#[test]
fn a_delegate_is_a_functional_interface() {
    let src = "delegate i32 Scale(i32 x);\n\
               i32 F() {\n    Scale twice = x => x * 2;\n    Func<i32, string, bool> f = (n, s) => n > 0;\n    Action<i32> log = n => { };\n    log(1);\n    return twice(3) + twice.Invoke(4);\n}\n";
    let out = java(src).unwrap_or_else(|p| panic!("{p:#?}"));
    for want in [
        "@FunctionalInterface",
        "public static interface Scale",
        "int Invoke(int x);",
        "dev.yog.script.Func3<Integer, String, Boolean> f",
        "dev.yog.script.Action1<Integer> log",
        "log.Invoke(1);",
        "twice.Invoke(3) + twice.Invoke(4)",
    ] {
        assert!(out.contains(want), "missing `{want}` in:\n{out}");
    }
}

#[test]
fn a_types_operators_are_static_methods() {
    let src = "public struct Energy(i32 amount) {\n    public i32 Amount = amount;\n\
               public static Energy operator +(Energy a, Energy b) => new Energy(a.Amount + b.Amount);\n\
               public static bool operator ==(Energy a, Energy b) => a.Amount == b.Amount;\n\
               public static bool operator !=(Energy a, Energy b) => a.Amount != b.Amount;\n\
               public static implicit operator Energy(i32 amount) => new Energy(amount);\n\
               public static explicit operator i32(Energy e) => e.Amount;\n}\n\
               Energy Sum(Energy a, Energy b) => a + b;\nbool Same(Energy a, Energy b) => a == b;\n\
               Energy Five() => 5;\ni32 Back(Energy e) => (i32)e;\n";
    let out = java(src).unwrap_or_else(|p| panic!("{p:#?}"));
    for want in [
        "public static Energy op_Addition(Energy a, Energy b)",
        "public static Energy op_Implicit(int amount)",
        "public static int op_Explicit(Energy e)",
        "return Energy.op_Addition(a.copy(), b.copy());",
        "return Energy.op_Equality(a.copy(), b.copy());",
        "return Energy.op_Implicit(5);",
        "return Energy.op_Explicit(e.copy());",
    ] {
        assert!(out.contains(want), "missing `{want}` in:\n{out}");
    }
}

#[test]
fn bits_and_steps_are_javas() {
    let src = "i32 F(i32 a, i32 b) => (a & b) | (a ^ ~b) << 2 >> 1;\n\
               u64 G(u64 x) => x >> 3;\n\
               i32 H() {\n    let mut i = 0;\n    i++;\n    --i;\n    i <<= 2;\n    return i++;\n}\n\
               public struct Mask(i32 bits) {\n    public i32 Bits = bits;\n\
               public static Mask operator &(Mask a, Mask b) => new Mask(a.Bits & b.Bits);\n\
               public static Mask operator ++(Mask a) => new Mask(a.Bits + 1);\n}\n\
               Mask K(Mask a) {\n    let mut m = a;\n    m++;\n    m &= a;\n    return ++m;\n}\n";
    let out = java(src).unwrap_or_else(|p| panic!("{p:#?}"));
    for want in [
        "return ((a & b) | (((a ^ (~b)) << 2) >> 1));",
        "return (x >>> 3);",
        "i++;",
        "--i;",
        "i <<= 2;",
        "return i++;",
        "m = Mask.op_Increment(m);",
        "m = Mask.op_BitwiseAnd(m, a.copy());",
        "return (m = Mask.op_Increment(m));",
    ] {
        assert!(out.contains(want), "missing `{want}` in:\n{out}");
    }
}

#[test]
fn collections_are_the_librarys() {
    let src = "i32 Sum(List<i32> xs) {\n    let mut total = 0;\n    foreach (let x in xs) {\n        total += x;\n    }\n    xs[0] = total;\n    return total + xs.Count + xs[1];\n}\n\
               bool Has(Dictionary<string, i32> ages, string name) => ages.ContainsKey(name) && ages[name] > 3;\n\
               i32 Count(IList<string> names) => names.Count;\n\
               List<i32> Made(List<i32> more) => [1, 2, ..more];\n\
               Dictionary<string, i32> Ages() => [\"steve\" => 3];\n\
               bool In(HashSet<string> set) => \"a\" in set;\n";
    let out = java(src).unwrap_or_else(|p| panic!("{p:#?}"));
    for want in [
        "public static int Sum(dev.yog.script.collections.generic.List<Integer> xs)",
        "for (var x : xs)",
        "xs.set_Item(0, total);",
        "xs.Count()",
        "xs.get_Item(1)",
        "ages.ContainsKey(name)",
        "names.Count()",
        "new dev.yog.script.collections.generic.List<Integer>();",
        ".Add(1);",
        ".Add(\"steve\", 3);",
        "set.Contains(\"a\")",
    ] {
        assert!(out.contains(want), "missing `{want}` in:\n{out}");
    }
}
