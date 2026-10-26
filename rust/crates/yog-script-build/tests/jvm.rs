//! Yog Script run on the JVM: what the compiled code computes.

mod support;

/// The harness itself: a project of two sources, one reaching the other's
/// type, `params` packed, an event raised once per handler.
#[test]
fn a_project_runs() {
    let size = "public record Size(i32 W, i32 H) {\n    public i32 Area() => W * H;\n}\n";
    let main = "public static class Calc {\n\
                    public static i32 Sum(params i32[] xs) { let mut t = 0; foreach (let x in xs) { t += x; } return t; }\n\
                    public static i32 Run() => new Size(3, 4).Area() + Sum(1, 2, 3);\n\
                }\n";
    let Some(out) = support::run(&[("size.ys", size), ("main.ys", main)], "t.MainScript.Calc.Run()") else {
        return;
    };
    assert_eq!(out, "18");
}

/// An event's argument is computed once, however many handlers it has.
#[test]
fn an_events_arguments_are_computed_once() {
    let src = "public class Counter {\n\
                   public event Action<i32> Hit;\n\
                   public i32 Calls = 0;\n\
                   public i32 Seen = 0;\n\
                   public i32 Next() { Calls += 1; return Calls; }\n\
                   public i32 Run() {\n\
                       Hit += x => { Seen += x; };\n\
                       Hit += x => { Seen += x; };\n\
                       Hit(Next());\n\
                       return Calls * 10 + Seen;\n\
                   }\n\
               }\n\
               public static class E { public static i32 Run() => new Counter().Run(); }\n";
    let Some(out) = support::run(&[("e.ys", src)], "t.EScript.E.Run()") else {
        return;
    };
    assert_eq!(out, "12", "one call of Next, both handlers saw 1");
}

/// A generic type's `T` in its constructor and its static members.
#[test]
fn type_parameters_in_constructors_and_statics() {
    let src = "public class Box<T> {\n\
                   public string Name;\n\
                   public Box() { Name = typeof(T).Name; }\n\
                   public static string Of() => typeof(T).Name;\n\
               }\n\
               public record Part(i32 Size);\n\
               public static class B { public static string Run() => new Box<Part>().Name + \" \" + Box<string>.Of(); }\n";
    let Some(out) = support::run(&[("b.ys", src)], "t.BScript.B.Run()") else {
        return;
    };
    assert_eq!(out, "Part String");
}

/// `T` handed on: a generic base's constructor given its own arguments
/// from the derived one's, `this(…)` handing them along, a static one
/// called in its type from an instance and a static member.
#[test]
fn type_parameters_handed_on() {
    let src = "public class Base<T> {\n\
                   public string Seen;\n\
                   public Base() { Seen = Label(); }\n\
                   public static string Label() => typeof(T).Name;\n\
                   public readonly string Again() => Label();\n\
                   public static string Twice() => Label() + Label();\n\
               }\n\
               public class Pair<TItem> : Base<TItem> {\n\
                   public i32 N;\n\
                   public Pair(i32 n) { N = n; }\n\
                   public Pair() : this(7) { }\n\
               }\n\
               public record Part(i32 Size);\n\
               public static class B {\n\
                   public static string Run() {\n\
                       let p = new Pair<Part>();\n\
                       return p.Seen + \" \" + p.Again() + \" \" + Base<i64>.Twice() + \" \" + p.N;\n\
                   }\n\
               }\n";
    let Some(out) = support::run(&[("b.ys", src)], "t.BScript.B.Run()") else {
        return;
    };
    assert_eq!(out, "Part Part Int64Int64 7");
}
