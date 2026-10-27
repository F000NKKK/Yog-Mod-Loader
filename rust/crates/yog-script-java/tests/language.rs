//! The language's own constructs compiled to Java: `with`, `ref` and
//! `out`, lambdas and method groups, generics kept, built-in macros,
//! quantities, `async` methods.

mod support;
use support::java;

/// `with`: a record made again of its components, a struct's copy set.
#[test]
fn with_makes_a_changed_copy() {
    let src = "record Pos(i32 X, i32 Y) {\n    public string Tag = \"t\";\n}\n\
               struct Pt {\n    public i32 X;\n}\n\
               Pos F(Pos p) => p with { Y = 64 };\nPt G(Pt p) => p with { X = 2 };\n";
    let out = java(src).unwrap_or_else(|p| panic!("{p:#?}"));
    for want in [
        "return new Pos($t1.X(), $t2, $t1.Tag());",
        "var $t3 = p.copy();",
        "$t3.X = $t4;",
    ] {
        assert!(out.contains(want), "missing `{want}` in:\n{out}");
    }
}

/// `ref` and `out`: a parameter a cell, an argument a cell of its place,
/// given back after the call.
#[test]
fn ref_and_out_pass_cells() {
    let src = "void Bump(ref i32 x) {\n    x += 1;\n}\n\
               bool Get(out i32 v) {\n    v = 2;\n    return true;\n}\n\
               i32 F() {\n    let mut a = 1;\n    Bump(ref a);\n    if (Get(out let v)) {\n        return a + v;\n    }\n    return a;\n}\n";
    let out = java(src).unwrap_or_else(|p| panic!("{p:#?}"));
    for want in [
        "public static void Bump(dev.yog.script.Ref<Integer> x) {",
        "x.value += 1;",
        "Bump($t1 = new dev.yog.script.Ref<>(a));",
        "a = $t1.value;",
        "int v;",
        "if (dev.yog.script.Ys.before(Get($t2 = new dev.yog.script.Ref<>()), v = $t2.value)) {",
    ] {
        assert!(out.contains(want), "missing `{want}` in:\n{out}");
    }
}

/// A trailing block lambda: `it` its parameter, its last expression its value.
#[test]
fn block_lambdas_give_their_last_value() {
    let src = "i32 Apply(i32 x, Func<i32, i32> f) => f(x);\n\
               i32 F() => Apply(4) {\n    let y = it + 1;\n    y * y\n};\n";
    let out = java(src).unwrap_or_else(|p| panic!("{p:#?}"));
    for want in [
        "Apply(4, (it) -> {",
        "final var y = (it + 1);",
        "return (y * y);",
    ] {
        assert!(out.contains(want), "missing `{want}` in:\n{out}");
    }
}

/// A method group: a name a lambda calling it, a member Java's reference.
#[test]
fn method_groups_are_delegates() {
    let src = "class Calc {\n    public i32 Times(i32 x) => x;\n    public static i32 Plus(i32 x) => x;\n}\n\
               i32 Twice(i32 x) => x * 2;\n\
               i32 F(Calc c) {\n    Func<i32, i32> f = Twice;\n    Func<i32, i32> g = c.Times;\n    Func<i32, i32> h = Calc.Plus;\n    return f(1) + g(2) + h(3);\n}\n";
    let out = java(src).unwrap_or_else(|p| panic!("{p:#?}"));
    for want in [
        "f = ($t1) -> Twice($t1);",
        "g = c::Times;",
        "h = Calc::Plus;",
    ] {
        assert!(out.contains(want), "missing `{want}` in:\n{out}");
    }
}

/// Generics kept: a method's type parameters handed to it, a type's read
/// off its object.
#[test]
fn type_parameters_are_values() {
    let src = "class Box<T> {\n    public string Kind() => typeof(T).Name;\n}\n\
               string Name<T>() => typeof(T).Name;\nstring F() => Name<i32>();\n";
    let out = java(src).unwrap_or_else(|p| panic!("{p:#?}"));
    for want in [
        "dev.yog.script.Type.argument(this, Box.class, 0).Name()",
        "public static <T> String Name(dev.yog.script.Type $typeT) {",
        "return $typeT.Name();",
        "Name(dev.yog.script.Type.Of(dev.yog.script.host.Int32.class))",
    ] {
        assert!(out.contains(want), "missing `{want}` in:\n{out}");
    }
}

/// The built-in macros: runtime helpers; `dbg!` shows its expression as written.
#[test]
fn builtin_macros_are_helpers() {
    let src = "i32 F(i32 rate) {\n    assert!(rate > 0);\n    assert_eq!(rate, 2);\n    return dbg!(rate * 2);\n}\n\
               void G() {\n    todo!();\n}\n";
    let out = java(src).unwrap_or_else(|p| panic!("{p:#?}"));
    for want in [
        "dev.yog.script.Ys.check((rate > 0), \"assertion failed\", \"Sample.ys:2\");",
        "dev.yog.script.Ys.checkEq(rate, 2, \"Sample.ys:3\");",
        "dev.yog.script.Ys.dbg((rate * 2), \"rate * 2\", \"Sample.ys:4\")",
        "dev.yog.script.Ys.panic(\"not done yet\", \"Sample.ys:7\");",
    ] {
        assert!(out.contains(want), "missing `{want}` in:\n{out}");
    }
}

/// Quantities: numbers of their unit, a multiple of seconds as seconds.
#[test]
fn units_are_numbers_of_their_unit() {
    let src =
        "f64 T(Ticks t) => t;\nf64 S(Seconds s) => s;\nf64 F() => T(20t) + S(250ms) + S(2m);\n";
    let out = java(src).unwrap_or_else(|p| panic!("{p:#?}"));
    for want in [
        "public static double T(double t) {",
        "T(20)",
        "S(0.25)",
        "S(120.0)",
    ] {
        assert!(out.contains(want), "missing `{want}` in:\n{out}");
    }
}

/// An `async` method: a machine of states, its locals across a wait its
/// fields, each `await` a state it goes on from.
#[test]
fn async_methods_are_machines() {
    let src = "using System.Threading.Tasks;\n\
               async Task<i32> F(i32 n) {\n    let x = n + 1;\n    await Task.Delay(1s);\n    return x;\n}\n";
    let out = java(src).unwrap_or_else(|p| panic!("{p:#?}"));
    for want in [
        "public static dev.yog.script.threading.tasks.Task1<Integer> F(int n) {",
        "extends dev.yog.script.threading.tasks.Async<Integer> {",
        "int n$p;",
        "x$v1 = (n$p + 1);",
        "if ($suspend($t1)) return;",
        "case 2:",
        "if (true) { $finish(x$v1); return; }",
        ".n$p = n;",
        "return $m$t2.$task();",
    ] {
        assert!(out.contains(want), "missing `{want}` in:\n{out}");
    }
}

/// An `await` only some paths reach: its value a field, the path not taken
/// jumped past.
#[test]
fn an_await_some_paths_reach_is_jumped_past() {
    let src = "using System.Threading.Tasks;\n\
               async Task<bool> Yes() {\n    await Task.Yield();\n    return true;\n}\n\
               async Task<bool> F(bool a) => a && await Yes();\n";
    let out = java(src).unwrap_or_else(|p| panic!("{p:#?}"));
    for want in ["= a$p;", "if (!$t", ") if (true) { $state = ", ".Result();"] {
        assert!(out.contains(want), "missing `{want}` in:\n{out}");
    }
}

/// `event`s: an `Event` of handlers, a method's removed by its key; a bus
/// one listened to and emitted; a `[Provide]` method answering when the
/// mods load.
#[test]
fn events_are_handlers() {
    let src = "using Yog.Native;\n\
               record Overloaded(i64 Energy);\n\
               [Event(\"charge:overloaded\")]\npublic static event Action<Overloaded> Overloads;\n\
               [Provide(\"charge:double\")]\nstatic i64 Double(i64 x) => x * 2;\n\
               class Button {\n    public event Action<string> Clicked;\n    public void Click() => Clicked(\"me\");\n    public void Saw(string s) { }\n}\n\
               void F(Overloaded o) {\n    let mut b = new Button();\n    b.Clicked += b.Saw;\n    b.Clicked -= b.Saw;\n    Overloads += e => { };\n    Overloads.Invoke(o);\n}\n";
    let checked = yog_script_analyze::check("Sample.ys", src);
    assert!(checked.problems.is_empty(), "{:#?}", checked.problems);
    let class = yog_script_java::compile("Sample.ys", src, &checked, "p")
        .unwrap_or_else(|p| panic!("{p:#?}"));
    let out = class.source;
    for want in [
        "public final dev.yog.script.Event<dev.yog.script.Action1<String>> Clicked = new dev.yog.script.Event<>();",
        // Raised by the file's raiser of its signature: no lambda, its
        // arguments computed once.
        "$raise1(Clicked, \"me\")",
        "private static void $raise1(dev.yog.script.Event<dev.yog.script.Action1<String>> ",
        ".handlers();",
        "b.Clicked.add(java.util.List.of(b, \"Saw#0\"), b::Saw);",
        "b.Clicked.remove(java.util.List.of(b, \"Saw#0\"));",
        "dev.yog.api.YogBus.on(\"charge:overloaded\", 0, Overloaded.class,",
        "dev.yog.api.YogBus.emit(\"charge:overloaded\", Overloaded.class, o)",
        "public record DoubleRequest(long x) {}",
        "dev.yog.api.YogBus.provide(\"charge:double\", SampleScript.DoubleRequest.class,",
    ] {
        assert!(out.contains(want), "missing `{want}` in:\n{out}");
    }
    assert!(
        class.mods.iter().any(|m| m == "p.SampleScript$$Load"),
        "{:?}",
        class.mods
    );
}

/// A local a lambda captures and something changes: a cell.
#[test]
fn a_changed_capture_is_a_cell() {
    let src = "i32 F() {\n    let mut n = 0;\n    Action add = () => { n += 1; };\n    add();\n    return n;\n}\n";
    let out = java(src).unwrap_or_else(|p| panic!("{p:#?}"));
    for want in [
        "final dev.yog.script.Ref<Integer> n = new dev.yog.script.Ref<>(0);",
        "n.value += 1;",
        "return n.value;",
    ] {
        assert!(out.contains(want), "missing `{want}` in:\n{out}");
    }
}

/// A command: its methods' class, a way of running per how many arguments
/// it's given, declared when the mods load.
#[test]
fn commands_are_ways_of_running() {
    let src = "command \"charge\" {\n    server string Fill(Sender source, i32 amount = 1) requires !source.IsConsole => \"filled \" + amount;\n}\n";
    let checked = yog_script_analyze::check("Sample.ys", src);
    assert!(checked.problems.is_empty(), "{:#?}", checked.problems);
    let class = yog_script_java::compile("Sample.ys", src, &checked, "p")
        .unwrap_or_else(|p| panic!("{p:#?}"));
    for want in [
        "public static final class ChargeCommand {",
        "static boolean $runFill0(dev.yog.script.yog.Sender source, java.util.List<String> $t",
        "int amount = 1;",
        "int amount = Integer.parseInt(",
        "if (!((!source.IsConsole()))) return false;",
        "source.Reply(Fill(source, amount));",
        "dev.yog.script.yog.Commands.declare(\"charge\", \"fill\", new String[] {  }, ChargeCommand::$runFill0);",
        "dev.yog.script.yog.Commands.declare(\"charge\", \"fill\", new String[] { \"int\" }, ChargeCommand::$runFill1);",
    ] {
        assert!(
            class.source.contains(want),
            "missing `{want}` in:\n{}",
            class.source
        );
    }
}

/// `t"key"` is Yog's `Text` of the key; its arguments go in `With`;
/// a `string` becomes one by `Text`'s implicit conversion.
#[test]
fn texts_are_yogs_text() {
    let src = "Text Full(i32 n) => t\"m.full\"(n);\nText Words() => \"plain\";\n";
    let checked = yog_script_analyze::check("T.ys", src);
    assert!(checked.problems.is_empty(), "{:#?}", checked.problems);
    let java = yog_script_java::compile("T.ys", src, &checked, "p")
        .unwrap_or_else(|p| panic!("{p:#?}"))
        .source;
    for want in [
        "dev.yog.script.yog.Text.Of(\"m.full\").With(new Object[] { n })",
        "dev.yog.script.yog.Text.op_Implicit(\"plain\")",
    ] {
        assert!(java.contains(want), "missing `{want}` in:\n{java}");
    }
}

/// A `params` parameter's arguments packed in an array of its item type;
/// an array given for it passed as it is.
#[test]
fn params_are_packed() {
    let src = "i32 Count(string f, params object[] xs) => xs.Length;\n\
               i32 A() => Count(\"x\", 1, \"two\");\ni32 B() => Count(\"x\");\ni32 C(object[] all) => Count(\"x\", move all);\n";
    let checked = yog_script_analyze::check("P.ys", src);
    assert!(checked.problems.is_empty(), "{:#?}", checked.problems);
    let java = yog_script_java::compile("P.ys", src, &checked, "p")
        .unwrap_or_else(|p| panic!("{p:#?}"))
        .source;
    for want in [
        "Count(\"x\", new Object[] { 1, \"two\" })",
        "Count(\"x\", new Object[] {  })",
        "Count(\"x\", all)",
    ] {
        assert!(java.contains(want), "missing `{want}` in:\n{java}");
    }
}
