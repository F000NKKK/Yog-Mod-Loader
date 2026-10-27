//! Sources compiled to Java: what's written, and what isn't yet.

use yog_ids::script_problem;
use yog_script_java::compile;

mod support;
use support::java;

#[test]
fn a_program_compiles() {
    let out = java(include_str!("Sample.ys")).unwrap_or_else(|p| panic!("{p:#?}"));
    for want in [
        "package yogscript.test;",
        "public final class SampleScript {",
        // One model of enum: a class of numbers, a case's data its subclass's.
        "public static sealed class Mode extends dev.yog.script.Enum permits Mode.Charging {",
        "public static final Mode Idle = TABLE.of(0L);",
        "public static final Mode Full = TABLE.of(2L);",
        "public static final class Charging extends Mode {",
        "super(1L);",
        "public int Rate() { return Rate; }",
        "public static final class Tier extends dev.yog.script.Enum {",
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
    let src = "i32 F() => await 1;\n";
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
    let src = include_str!("../../yog-script-analyze/tests/fixtures/Glassy.ys");
    let checked = yog_script_analyze::check("Glassy.ys", src);
    let out = compile("Glassy.ys", src, &checked, "yogscript.glassy")
        .map(|c| c.source)
        .unwrap_or_else(|p| panic!("{p:#?}"));
    for want in [
        "public static final String Prefix = \"g\";",
        "dev.yog.bus.YogEvents.Block.Break.on(0, (e) -> {",
        "if (dev.yog.script.host.String.StartsWith(e.playerName(), Prefix)) {",
        "new dev.yog.bus.YogServices.World.SetBlock(e.dimension(), e.pos(), \"minecraft:glass\").call();",
        "return null;",
    ] {
        assert!(out.contains(want), "missing `{want}` in:\n{out}");
    }
}

/// A method as an event's handler: made once, called with each event.
#[test]
fn a_method_handles_an_event() {
    let src = "mod Glassy {\n    Version = \"1.0.0\";\n    Targets = mc 1.20.1..26.2;\n\
               Glassy() {\n        Block.Break += OnBreak;\n    }\n\
               void OnBreak(Block.BreakArgs e) { }\n}\n";
    let checked = yog_script_analyze::check("Glassy.ys", src);
    let out = compile("Glassy.ys", src, &checked, "yogscript.glassy")
        .map(|c| c.source)
        .unwrap_or_else(|p| panic!("{p:#?}"));
    for want in [
        "= ($t3) -> OnBreak($t3);",
        "dev.yog.bus.YogEvents.Block.Break.on(0, ($t2) -> {",
        "$t1.Invoke($t2);",
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
               Mask K(Mask a) {\n    let mut m = a;\n    m++;\n    m &= a;\n    return ++m;\n}\n\
               Mask Old(Mask a) {\n    let mut m = a;\n    return m++;\n}\n";
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
        // A postfix's value is the old one.
        "return dev.yog.script.Ys.before(m, m = Mask.op_Increment(m));",
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
        "dev.yog.script.Type.made(new dev.yog.script.collections.generic.List<Integer>(), $kept",
        ".Add(1);",
        ".Add(\"steve\", 3);",
        "set.Contains(\"a\")",
    ] {
        assert!(out.contains(want), "missing `{want}` in:\n{out}");
    }
}

#[test]
fn a_strings_members_are_its_companions() {
    let src = "string F(string s) => s.Trim().Substring(1, s.Length - 2);\n\
               char G(string? s) => s?.Length > 0 ? s![0] : 'x';\n\
               string J(List<string> parts) => String.Join(\", \", in parts) + String.Empty;\n";
    let out = java(src).unwrap_or_else(|p| panic!("{p:#?}"));
    for want in [
        "dev.yog.script.host.String.Substring(dev.yog.script.host.String.Trim(s), 1, (dev.yog.script.host.String.Length(s) - 2))",
        "dev.yog.script.host.String.get_Item(",
        "dev.yog.script.host.String.Join(\", \", parts)",
        "dev.yog.script.host.String.Empty",
    ] {
        assert!(out.contains(want), "missing `{want}` in:\n{out}");
    }
}

#[test]
fn arrays_are_javas() {
    let src = "i32 Grid() {\n    let g = new i32[3, 4];\n    g[1, 2] = 5;\n    let mut n = 0;\n    foreach (let v in g) {\n        n += v;\n    }\n    return g.GetLength(1) + g[1, 2] + n;\n}\n\
               string[] Names() => new[] { \"a\", \"b\" };\n\
               i32[] Made(List<i32> more) => [1, 2, ..more];\n\
               void Sorted(i32[] xs) {\n    let mut ys = move xs;\n    Array.Sort(ref ys);\n}\n\
               i32 Count(in IEnumerable<i32> xs) => 0;\n\
               i32 Viewed(i32[] xs) => Count(in xs);\n\
               i32[] Back(List<i32> xs) => xs.ToArray();\n\
               string[] Grown() {\n    string[]? names = null;\n    Array.Resize(ref names, 3);\n    return names!;\n}\n";
    let out = java(src).unwrap_or_else(|p| panic!("{p:#?}"));
    for want in [
        "new int[3][4]",
        // A null `T[]?` resized: generic code made it, the place's type it's made.
        "names = dev.yog.script.host.Array.Of(dev.yog.script.host.Array.<String>Resize(names, 3), String[].class);",
        "g[1][2] = 5;",
        "for (var v : $t",
        "dev.yog.script.host.Array.GetLength(g, 2, 1)",
        "new String[] { \"a\", \"b\" }",
        "dev.yog.script.host.Array.Of($t",
        "ys = dev.yog.script.host.Array.<Integer>Sort(ys);",
        "Count(dev.yog.script.host.Array.AsList(xs))",
        "dev.yog.script.host.Array.Of(xs.ToArray(), int[].class)",
    ] {
        assert!(out.contains(want), "missing `{want}` in:\n{out}");
    }
}

#[test]
fn types_are_described_beside_them() {
    let src = "namespace My.Mod;\ninterface IShape {}\nclass Box<T> : IShape {\n    public T Item;\n}\n\
               bool Same(object o) => o.GetType() == typeof(Box<i32>);\n\
               Type Array() => typeof(string[]);\n\
               Type Small(u8 b) => b.GetType();\n";
    let out = java(src).unwrap_or_else(|p| panic!("{p:#?}"));
    for want in [
        // Its description: its name as declared, what it is, its parameters, its bases.
        "public static final dev.yog.script.Type $TYPE = dev.yog.script.Type.Declare(Box.class, \"My.Mod.Box\", dev.yog.script.Type.CLASS, new String[] { \"T\" }, Box::$describe);",
        "d.Interface(dev.yog.script.Type.Of(IShape.class));",
        "dev.yog.script.Type.Declare(IShape.class, \"My.Mod.IShape\", dev.yog.script.Type.INTERFACE",
        "dev.yog.script.host.Object.GetType(o)",
        "dev.yog.script.Type.Of(Box.class).MakeGenericType(new dev.yog.script.Type[] { dev.yog.script.Type.Of(dev.yog.script.host.Int32.class) })",
        "dev.yog.script.Type.Of(dev.yog.script.host.String.class).MakeArrayType(1)",
        // A number's: as declared, Java's `int` can't say `u8`.
        "dev.yog.script.Ys.before(dev.yog.script.Type.Of(dev.yog.script.host.Byte.class), b)",
    ] {
        assert!(out.contains(want), "missing `{want}` in:\n{out}");
    }
}

#[test]
fn members_are_described_with_their_uses() {
    let src = "class Box<T> {\n    public T Item;\n    public const i32 Max = 9;\n    public string Label => \"box\";\n\
               public Box(T item) {\n        Item = item;\n    }\n\
               public void Set(T item, i32 times = 1) {\n        Item = item;\n    }\n}\n\
               record Pos(i32 X, i32 Y);\nenum Mode { Idle, Run }\n";
    let out = java(src).unwrap_or_else(|p| panic!("{p:#?}"));
    for want in [
        "Box::$describe);",
        "private static void $describe(dev.yog.script.reflection.TypeDescription d) {",
        // Each member with what reads, writes or calls it, as code uses it.
        "d.Field(\"Item\", 0, d.Parameter(0), o -> ((Box) o).Item, (o, v) -> ((Box) o).Item = v);",
        "d.Field(\"Max\", dev.yog.script.reflection.MemberInfo.STATIC | dev.yog.script.reflection.MemberInfo.LITERAL, dev.yog.script.Type.Of(dev.yog.script.host.Int32.class), o -> Box.Max, null);",
        "d.Property(\"Label\", 0, dev.yog.script.Type.Of(dev.yog.script.host.String.class), o -> ((Box) o).Label(), null);",
        "d.Constructor(0, a -> new Box((dev.yog.script.Type[]) null, a[0])).Takes(\"item\", d.Parameter(0), false);",
        "d.Method(\"Set\", 0, new String[] {  }, (o, a) -> { ((Box) o).Set(a[0], (Integer) a[1]); return null; })",
        ".Takes(\"times\", dev.yog.script.Type.Of(dev.yog.script.host.Int32.class), true);",
        "d.Property(\"X\", 0, dev.yog.script.Type.Of(dev.yog.script.host.Int32.class), o -> ((Pos) o).X(), null);",
        "d.Field(\"Idle\", dev.yog.script.reflection.MemberInfo.STATIC | dev.yog.script.reflection.MemberInfo.LITERAL, dev.yog.script.Type.Of(Mode.class), o -> Mode.Idle, null);",
    ] {
        assert!(out.contains(want), "missing `{want}` in:\n{out}");
    }
}

#[test]
fn attributes_are_made_when_asked_for() {
    let src = "using System.Reflection;\n\
               public class Range(i32 min, i32 max) : Attribute {\n    public i32 Max = max;\n}\n\
               [Obsolete(\"use Charge\")]\n[Derive(Clone)]\nclass Charger {\n    [Range(0, 100)]\n    public i32 Level;\n}\n\
               i32 Top() => typeof(Charger).GetField(\"Level\")!.GetCustomAttribute<Range>()!.Max;\n";
    let out = java(src).unwrap_or_else(|p| panic!("{p:#?}"));
    for want in [
        // On the type, before its members; on a member, after it.
        "d.Attribute(() -> new dev.yog.script.Obsolete(\"use Charge\"));",
        "(o, v) -> ((Charger) o).Level = (Integer) v).Attribute(() -> new Range(0, 100));",
        // Its class handed first: what it's filtered by.
        "dev.yog.script.reflection.CustomAttributeExtensions.GetCustomAttribute(Range.class, ",
    ] {
        assert!(out.contains(want), "missing `{want}` in:\n{out}");
    }
    // `[Derive(Clone)]` names macros: no value to make.
    assert!(!out.contains("new dev.yog.script.Derive"), "{out}");
}

#[test]
fn a_quoted_lambda_is_its_tree_and_its_code() {
    let src = "using System.Linq.Expressions;\n\
               class Pos(i32 x) {\n    public i32 X = x;\n    public i32 Twice() => X * 2;\n}\n\
               Expression<Func<Pos, bool>> Far() => p => p.X + p.Twice() > 10;\n\
               bool Run(Pos p) => Far().Compile()(p);\n";
    let out = java(src).unwrap_or_else(|p| panic!("{p:#?}"));
    for want in [
        "dev.yog.script.linq.expressions.Expression.quote(",
        "new String[] { \"p\" }, new dev.yog.script.Type[] { dev.yog.script.Type.Of(Pos.class) }, $ps0 -> ",
        // Its nodes: members by name, calls by the method chosen.
        "dev.yog.script.linq.expressions.Expression.GreaterThan(dev.yog.script.linq.expressions.Expression.Add(dev.yog.script.linq.expressions.Expression.PropertyOrField($ps0[0], \"X\"), ",
        "dev.yog.script.linq.expressions.Expression.methodOf(dev.yog.script.Type.Of(Pos.class), \"Twice\", 0)",
        // Its code: what `Compile()` gives.
        "(dev.yog.script.Func2<Pos, Boolean>) (p) -> {",
        "Far().Compile().Invoke(p)",
    ] {
        assert!(out.contains(want), "missing `{want}` in:\n{out}");
    }
}

#[test]
fn enums_are_numbers_their_cases_data() {
    let src = "[Flags]\nenum Access : u8 { None = 0, Read = 1, Write = 2, Both = Read | Write }\n\
               enum Maybe<T> { Nothing, Just(T Value) }\n\
               Access Rw() => Access.Read | Access.Write;\n\
               i32 Num(Access a) => (i32)a;\n\
               Access Back(i32 n) => (Access)n;\n\
               bool Less(Access a, Access b) => a < b;\n\
               Maybe<i32> Three() => Maybe<i32>.Just(3);\n\
               i32 Or(Maybe<i32> m) => m switch {\n    Maybe<i32>.Just(let v) => v,\n    _ => 0,\n};\n";
    let out = java(src).unwrap_or_else(|p| panic!("{p:#?}"));
    for want in [
        "new long[] { 0L, 1L, 2L, 3L }).Width(8, false).Flags();",
        "Access.of(Access.Read.value() | Access.Write.value())",
        "((int) a.value())",
        "Access.of((long) n)",
        "(dev.yog.script.Enum.compare(a, b) < 0)",
        "TABLE = new dev.yog.script.Enum.Table<>(Maybe.class, Maybe::new, new String[] { \"Nothing\", \"Just\" }, new long[] { 0L, 1L }).Width(32, true).Carrying(1);",
        "new Maybe.Just<>(3)",
        // A generic case tested with what's matched's arguments: its field typed.
        "instanceof Maybe.Just<Integer> ",
    ] {
        assert!(out.contains(want), "missing `{want}` in:\n{out}");
    }
    // After `_ =>` nothing is reached: no panic written for no arm.
    assert!(!out.contains("noArm"), "{out}");
}

#[test]
fn a_tree_has_news_text_lambdas_and_operators() {
    let src = "using System.Linq.Expressions;\n\
               struct Money(i32 cents) {\n    public i32 Cents = cents;\n\
               public static Money operator +(Money a, Money b) => new Money(a.Cents + b.Cents);\n}\n\
               class Pos(i32 x) {\n    public i32 X = x;\n}\n\
               Expression<Func<i32, Pos>> Made() => x => new Pos(x);\n\
               Expression<Func<i32, string>> Text() => x => \"x is {x}\";\n\
               Expression<Func<List<i32>, i32, bool>> Any() => (xs, n) => xs.Any(y => y > n);\n\
               Expression<Func<Money, Money, Money>> Sum() => (a, b) => a + b;\n\
               i32[] Items(List<i32> xs) => [xs[0], xs[1]];\n";
    let out = java(src).unwrap_or_else(|p| panic!("{p:#?}"));
    for want in [
        "Expression.New(dev.yog.script.linq.expressions.Expression.constructorOf(dev.yog.script.Type.Of(Pos.class), 0), new dev.yog.script.linq.expressions.Expression[] { $ps0[0] })",
        "Expression.Add(dev.yog.script.linq.expressions.Expression.Constant((Object) \"x is \", ",
        // A lambda inside: its own parameters, the outer ones seen.
        "Expression.nested(dev.yog.script.Func2.class, ",
        "$ps1 -> dev.yog.script.linq.expressions.Expression.GreaterThan($ps1[0], $ps0[1])",
        // A type's operator: its method.
        "Expression.MakeBinary(dev.yog.script.linq.expressions.ExpressionType.Add, $ps0[0], $ps0[1], false, dev.yog.script.linq.expressions.Expression.operatorOf(dev.yog.script.Type.Of(Money.class), \"op_Addition\", ",
        // A collection's items written themselves: typed.
        "new int[] { xs.get_Item(0), xs.get_Item(1) }",
    ] {
        assert!(out.contains(want), "missing `{want}` in:\n{out}");
    }
}

#[test]
fn generic_objects_keep_their_arguments() {
    let src = "class Box<T>(T item) {\n    public T Item = item;\n}\n\
               object Made() => new Box<i32>(1);\n\
               object Names() => new List<string>();\n\
               Type Nullable() => typeof(i32?);\n";
    let out = java(src).unwrap_or_else(|p| panic!("{p:#?}"));
    for want in [
        "implements dev.yog.script.Typed",
        "private dev.yog.script.Type[] $types;",
        "new Box<Integer>($kept1, 1)",
        // Each list of arguments a constant of the class, worked out once.
        "private static final dev.yog.script.Type[] $kept1 = new dev.yog.script.Type[] { dev.yog.script.Type.Of(dev.yog.script.host.Int32.class) };",
        // The runtime's collections keep theirs too.
        "dev.yog.script.Type.made(new dev.yog.script.collections.generic.List<String>(), $kept2)",
        // A value type's `?` is `Nullable<T>`.
        "dev.yog.script.Type.Of(dev.yog.script.Nullable.class).MakeGenericType(",
    ] {
        assert!(out.contains(want), "missing `{want}` in:\n{out}");
    }
}

#[test]
fn patterns_are_conditions() {
    let src = "enum Mode { Idle, Charging(i32 Rate), Full }\n\
               class Monster(i32 health) {\n    public i32 Health = health;\n}\n\
               string Kind(i32 n) => n switch {\n    0 or 1 => \"small\",\n    > 1 and < 10 => \"mid\",\n    _ => \"big\",\n};\n\
               string Pair((i32, string) p) => p switch {\n    (0, let s) => s,\n    _ => \"other\",\n};\n\
               bool Weak(object o) => o is Monster { Health: < 5 };\n\
               i32 Fast(Mode m) {\n    if (m is Mode.Charging(let r) && r > 3) {\n        return r;\n    }\n    return 0;\n}\n";
    let out = java(src).unwrap_or_else(|p| panic!("{p:#?}"));
    for want in [
        "if ((java.util.Objects.equals($t",
        ".Item1, 0)) {",
        "instanceof Monster $t",
        // A name an `is` binds: declared before, assigned in the condition.
        "int r;",
        "dev.yog.script.Ys.bound(r = $t",
    ] {
        assert!(out.contains(want), "missing `{want}` in:\n{out}");
    }
}

#[test]
fn a_local_may_be_set_after() {
    let src = "i32 F(bool b) {\n    i32 x;\n    if (b) {\n        x = 1;\n    } else {\n        x = 2;\n    }\n    return x;\n}\n";
    let out = java(src).unwrap_or_else(|p| panic!("{p:#?}"));
    assert!(out.contains("int x;"), "{out}");
}

#[test]
fn foreach_takes_items_apart() {
    let src = "i32 Sum(Dictionary<string, i32> ages, List<(i32, string)> pairs) {\n    let mut t = 0;\n\
               \x20   foreach (let (name, age) in ages) {\n        t += age;\n    }\n\
               \x20   foreach (let (n, s) in pairs) {\n        t += n;\n    }\n    return t;\n}\n";
    let out = java(src).unwrap_or_else(|p| panic!("{p:#?}"));
    for want in [".Key();", ".Value();", ".Item1;", ".Item2;"] {
        assert!(out.contains(want), "missing `{want}` in:\n{out}");
    }
}

#[test]
fn a_loop_whose_condition_takes_steps() {
    let src = "enum Mode { Go, Stop }\nMode At(i32 i) => i < 3 ? Mode.Go : Mode.Stop;\n\
               i32 F() {\n    let mut n = 0;\n\
               \x20   for (let mut i = 0; At(i) switch { Mode.Go => true, Mode.Stop => false }; i += 1) {\n\
               \x20       if (i == 1) {\n            continue;\n        }\n        n += i;\n    }\n    return n;\n}\n";
    let out = java(src).unwrap_or_else(|p| panic!("{p:#?}"));
    // Checked each time, the body a block `continue` leaves to the step.
    for want in ["while (true) {", "if (!($t", ") break;", "break $t"] {
        assert!(out.contains(want), "missing `{want}` in:\n{out}");
    }
}

#[test]
fn a_records_own_fields() {
    let src = "record Pos(i32 X, i32 Y) {\n    public i32 Sum = X + Y;\n}\n";
    let out = java(src).unwrap_or_else(|p| panic!("{p:#?}"));
    for want in [
        "public record Pos(int X, int Y, int Sum)",
        "public Pos(int X, int Y) { this(X, Y, (X + Y)); }",
        // Its text as .NET writes a record's.
        "\"Pos { \" + \"X = \" + X + \", \" + \"Y = \" + Y + \", \" + \"Sum = \" + Sum + \" }\"",
    ] {
        assert!(out.contains(want), "missing `{want}` in:\n{out}");
    }
}

#[test]
fn variance_is_a_recast() {
    let out = java("IEnumerable<object> Out(List<string> xs) => xs;\n")
        .unwrap_or_else(|p| panic!("{p:#?}"));
    let want = "return ((dev.yog.script.collections.generic.IEnumerable<Object>) (Object) xs);";
    assert!(out.contains(want), "missing `{want}` in:\n{out}");
}

#[test]
fn identity_is_the_types() {
    let src = "public struct Pos(i32 x, i32 y) {\n    public i32 X = x;\n    public i32 Y = y;\n}\n\
               public class Tag(string name) {\n    public string Name = name;\n\
               public override i32 GetHashCode() => 7;\n    public override string ToString() => Name;\n}\n";
    let out = java(src).unwrap_or_else(|p| panic!("{p:#?}"));
    for want in [
        "return other instanceof Pos that && java.util.Objects.equals(X, that.X) && java.util.Objects.equals(Y, that.Y);",
        "return java.util.Objects.hash(X, Y);",
        "public int hashCode() {",
        "return GetHashCode();",
        "public String toString() {",
        "return ToString();",
    ] {
        assert!(out.contains(want), "missing `{want}` in:\n{out}");
    }
}

#[test]
fn an_interfaces_property_is_a_getter() {
    let src = "public interface IHas {\n    i32 Count { get; }\n}\n\
               public class Box(i32 count) : IHas {\n    public i32 Count { get; } = count;\n}\n\
               i32 F(Box b, IHas h) => b.Count + h.Count;\n";
    let out = java(src).unwrap_or_else(|p| panic!("{p:#?}"));
    for want in [
        "int Count();",
        "public int Count;",
        "public int Count() {",
        "return this.Count;",
        "h.Count()",
    ] {
        assert!(out.contains(want), "missing `{want}` in:\n{out}");
    }
}

#[test]
fn members_java_cant_tell_apart_are_named_apart() {
    let src = "bool Added(HashSet<string> set) => set.Add(\"a\");\n\
               void Through(ICollection<string> c) {\n    c.Add(\"b\");\n}\n\
               bool Gone(Dictionary<string, i32> d) => d.Remove(\"a\");\n\
               public interface IShape {\n    f64 Area();\n}\n\
               public class Square(f64 side) : IShape {\n    public f64 Side = side;\n    f64 IShape.Area() => Side * Side;\n}\n";
    let out = java(src).unwrap_or_else(|p| panic!("{p:#?}"));
    for want in [
        "return set.ISet$Add(\"a\");",
        "c.Add(\"b\");",
        "return d.IDictionary$Remove(\"a\");",
        "public double Area() {",
    ] {
        assert!(out.contains(want), "missing `{want}` in:\n{out}");
    }
}

#[test]
fn linq_is_enumerables_statics() {
    let src = "record Player(string Name, f64 Score);\n\
               List<string> High(List<Player> ps) => ps.Where(p => p.Score > 1.0).OrderBy(p => p.Name).Select(p => p.Name).ToList();\n\
               f64 Scores(List<Player> ps) => ps.Sum(p => p.Score);\n\
               f64 Mean(i32[] xs) => xs.Average();\n";
    let out = java(src).unwrap_or_else(|p| panic!("{p:#?}"));
    for want in [
        "dev.yog.script.linq.Enumerable.ToList(dev.yog.script.linq.Enumerable.Select(dev.yog.script.linq.Enumerable.OrderBy(dev.yog.script.linq.Enumerable.Where(ps,",
        "dev.yog.script.linq.Enumerable.Sum$3(ps,",
        "dev.yog.script.linq.Enumerable.Average(dev.yog.script.host.Array.AsList(xs))",
    ] {
        assert!(out.contains(want), "missing `{want}` in:\n{out}");
    }
}

#[test]
fn the_runtimes_properties_are_got_and_set() {
    let src = "i32 Linked() {\n    let mut list = new LinkedList<i32>();\n    let mut node = list.AddLast(2);\n    node.Value = 7;\n    return list.First!.Value;\n}\n\
               i32 Order(i32 a, i32 b) => Comparer<i32>.Default.Compare(a, b);\n\
               i32 Least(SortedSet<i32> set) => set.Min;\n";
    let out = java(src).unwrap_or_else(|p| panic!("{p:#?}"));
    for want in [
        "node.set_Value(7);",
        "list.First().Value()",
        "dev.yog.script.collections.generic.Comparer.Default().Compare(a, b)",
        "return set.Min();",
    ] {
        assert!(out.contains(want), "missing `{want}` in:\n{out}");
    }
}

#[test]
fn a_reified_method_is_handed_its_types() {
    let src = "List<string> Texts(List<object> things) => things.OfType<string>().ToList();\n\
               i32 Firsts(List<i32> xs) {\n    let mut n = 0;\n    foreach (let c in xs.Chunk(3)) {\n        n += c[0];\n    }\n    return n;\n}\n";
    let out = java(src).unwrap_or_else(|p| panic!("{p:#?}"));
    for want in [
        "((dev.yog.script.collections.generic.IEnumerable<String>) (Object) dev.yog.script.linq.Enumerable.OfType(String.class, things))",
        "((dev.yog.script.collections.generic.IEnumerable<int[]>) (Object) dev.yog.script.linq.Enumerable.Chunk(int.class, xs, 3))",
    ] {
        assert!(out.contains(want), "missing `{want}` in:\n{out}");
    }
}

#[test]
fn numbers_members_are_their_companions() {
    let src = "i32 F(string s) => i32.Parse(s) + Int32.MaxValue;\n\
               bool G(char c) => char.IsDigit(c);\n\
               decimal P(decimal a, i32 n) => -(a + n) / 2;\n";
    let out = java(src).unwrap_or_else(|p| panic!("{p:#?}"));
    for want in [
        "dev.yog.script.host.Int32.Parse(s) + dev.yog.script.host.Int32.MaxValue",
        "dev.yog.script.host.Char.IsDigit(c)",
        "a.add(dev.yog.script.host.Decimal.op_Implicit(n)).negate().divide(dev.yog.script.host.Decimal.op_Implicit(2), java.math.MathContext.DECIMAL128)",
    ] {
        assert!(out.contains(want), "missing `{want}` in:\n{out}");
    }
}

#[test]
fn random_text_and_lazy_are_the_runtimes() {
    let src = "void Mix(Random rng, i32[] xs) {\n    let mut ys = move xs;\n    rng.Shuffle(ref ys);\n}\n\
               string Text() {\n    let mut b = new StringBuilder();\n    b.Append(2.5);\n    b.Length = 1;\n    return b.ToString();\n}\n\
               i32 Later() => new Lazy<i32>(() => 42).Value;\n\
               string Say(f64 x, bool b) => \"x = {x}, \" + b;\n";
    let out = java(src).unwrap_or_else(|p| panic!("{p:#?}"));
    for want in [
        "ys = rng.Shuffle(ys);",
        "b.set_Length(1);",
        "new dev.yog.script.Lazy<Integer>($kept1, () -> {",
        "dev.yog.script.host.Double.ToString(x)",
        "dev.yog.script.host.Boolean.ToString(b)",
    ] {
        assert!(out.contains(want), "missing `{want}` in:\n{out}");
    }
}

#[test]
fn date_times_and_system_names_are_javas() {
    let src = "TimeSpan Age(DateTime born) => DateTime.Now - born;\n\
               Void Nothing() {}\n\
               Int32 Count(String s) => s.Length;\n";
    let out = java(src).unwrap_or_else(|p| panic!("{p:#?}"));
    for want in [
        "dev.yog.script.DateTime.op_Subtraction(dev.yog.script.DateTime.Now().copy(), born.copy())",
        "public static void Nothing()",
        "public static int Count(String s)",
    ] {
        assert!(out.contains(want), "missing `{want}` in:\n{out}");
    }
}

#[test]
fn tuples_are_value_tuples() {
    let src = "(i32 Min, i32 Max) Bounds(in List<i32> xs) => (xs.Min(), xs.Max());\n\
               i32 Width(List<i32> xs) {\n    let b = Bounds(in xs);\n    let (lo, hi) = b;\n    return b.Max - lo + hi;\n}\n\
               (string, i64) Named() => (\"a\", 1);\n";
    let out = java(src).unwrap_or_else(|p| panic!("{p:#?}"));
    for want in [
        "public static dev.yog.script.ValueTuple2<Integer, Integer> Bounds(",
        "return new dev.yog.script.ValueTuple2<>(",
        "final var lo = $t",
        "b.Item2",
        "new dev.yog.script.ValueTuple2<>(\"a\", 1L)",
    ] {
        assert!(out.contains(want), "missing `{want}` in:\n{out}");
    }
}

#[test]
fn a_query_is_linqs_calls() {
    let src = "record Player(string Name, i32 Level);\n\
               IEnumerable<string> High(List<Player> ps) => from p in ps where p.Level > 10 orderby p.Name select p.Name;\n\
               IEnumerable<i32> Pairs(i32[] xs, i32[] ys) => from x in xs from y in ys select x * y;\n";
    let out = java(src).unwrap_or_else(|p| panic!("{p:#?}"));
    for want in [
        "dev.yog.script.linq.Enumerable.Select(dev.yog.script.linq.Enumerable.OrderBy(dev.yog.script.linq.Enumerable.Where(ps, (p) -> {",
        "dev.yog.script.linq.Enumerable.SelectMany(dev.yog.script.host.Array.AsList(xs), (x) -> {",
        "final var x = $t",
    ] {
        assert!(out.contains(want), "missing `{want}` in:\n{out}");
    }
}
