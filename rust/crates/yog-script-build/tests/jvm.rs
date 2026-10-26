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
