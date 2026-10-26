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

/// `await` in a `switch`'s arms, under `try` and in `with`: each written in
/// the machine, the values kept in fields across the wait.
#[test]
fn awaits_in_switches_tries_and_withs() {
    let src = "using System.Threading.Tasks;\n\
               public record Part(i32 Size);\n\
               public static class A {\n\
                   static async Task<i32> Two() => await Task.FromResult(2);\n\
                   public static async Task<i32> Pick(i32 n) {\n\
                       let r = n switch { 1 => await Two(), 2 => await Two() * 10, let k => k };\n\
                       return r;\n\
                   }\n\
                   static Result<i32, Error> Five() => Ok(5);\n\
                   static Result<i32, Error> Bad() => Err(new Error(\"no\"));\n\
                   public static async Task<Result<i32, Error>> Tried(bool good) {\n\
                       let x = try await Task.FromResult(good ? Five() : Bad());\n\
                       return Ok(x + 1);\n\
                   }\n\
                   public static async Task<i32> Withed() {\n\
                       let p = new Part(1) with { Size = await Two() };\n\
                       return p.Size;\n\
                   }\n\
                   public static string Run() => Pick(1).Result + \" \" + Pick(2).Result + \" \" + Pick(7).Result\n\
                       + \" \" + Withed().Result + \" \" + Tried(true).Result.Unwrap() + \" \" + Tried(false).Result.IsErr();\n\
               }\n";
    let Some(out) = support::run(&[("a.ys", src)], "t.AScript.A.Run()") else {
        return;
    };
    assert_eq!(out, "2 20 7 2 6 True");
}

/// `await` around `?.`: the receiver awaited, then read when it isn't
/// `null`; a call's argument awaited only when the receiver isn't.
#[test]
fn awaits_around_safe_access() {
    let src = "using System.Threading.Tasks;\n\
               public class Box(i32 size) {\n\
                   public i32 Size = size;\n\
                   public i32 Plus(i32 n) => Size + n;\n\
               }\n\
               public static class S {\n\
                   public static i32 Waited = 0;\n\
                   static async Task<Box?> Maybe(bool some) => await Task.FromResult(some ? new Box(3) : null);\n\
                   static async Task<i32> Two() { Waited += 1; return await Task.FromResult(2); }\n\
                   public static async Task<i32> Read(bool some) => (await Maybe(some))?.Size ?? -1;\n\
                   public static async Task<i32> Call(Box? b) => b?.Plus(await Two()) ?? -1;\n\
                   public static string Run() => Read(true).Result + \" \" + Read(false).Result + \" \"\n\
                       + Call(new Box(5)).Result + \" \" + Call(null).Result + \" \" + Waited;\n\
               }\n";
    let Some(out) = support::run(&[("s.ys", src)], "t.SScript.S.Run()") else {
        return;
    };
    assert_eq!(out, "3 -1 7 -1 1", "the argument awaited once, for the box there");
}

/// `?.` calls, as C#'s: nothing called (its arguments not computed) on
/// `null`, the value `null` then; a `void` one skipped.
#[test]
fn safe_calls_skip_null() {
    let src = "public class Box(i32 size) {\n\
                   public i32 Size = size;\n\
                   public readonly i32 Plus(i32 n) => Size + n;\n\
                   public void Grow() { Size += 1; }\n\
               }\n\
               public static class C {\n\
                   public static i32 Made = 0;\n\
                   static i32 Arg() { Made += 1; return 10; }\n\
                   public static string Run() {\n\
                       Box? none = null;\n\
                       let mut some = new Box(1);\n\
                       none?.Grow();\n\
                       some?.Grow();\n\
                       return (none?.Plus(Arg()) ?? -1) + \" \" + (some?.Plus(Arg()) ?? -1) + \" \" + Made;\n\
                   }\n\
               }\n";
    let Some(out) = support::run(&[("c.ys", src)], "t.CScript.C.Run()") else {
        return;
    };
    assert_eq!(out, "-1 12 1");
}

/// Threads, locks and atomics: work split over threads adds up, a lock's
/// value changed by each in turn, scoped threads joined, once set once.
#[test]
fn threads_locks_and_atomics() {
    let src = "using System.Threading;\n\
               public static class P {\n\
                   public static string Run() {\n\
                       let hits = new Atomic<i32>(0);\n\
                       let total = new Lock<i64>(0);\n\
                       let mut handles = new List<ThreadHandle<i32>>();\n\
                       for (let mut t = 0; t < 4; t += 1) {\n\
                           handles.Add(Thread.Run(() => {\n\
                               for (let mut i = 0; i < 1000; i += 1) {\n\
                                   hits.Increment();\n\
                                   using (let mut g = total.Enter()) { g.Value = g.Value + 1; }\n\
                               }\n\
                               return 1;\n\
                           }));\n\
                       }\n\
                       let mut joined = 0;\n\
                       foreach (let h in handles) { joined += h.Join(); }\n\
                       let scoped = new Atomic<i32>(0);\n\
                       Thread.Scope(s => {\n\
                           s.Run(() => { scoped.Add(10); });\n\
                           s.Run(() => { scoped.Add(5); });\n\
                       });\n\
                       let once = new Once<string>();\n\
                       let first = once.TrySet(\"a\");\n\
                       let second = once.TrySet(\"b\");\n\
                       let cell = new Cell<i32>(1);\n\
                       cell.Update(x => x * 7);\n\
                       let mut g = total.Enter();\n\
                       let sum = g.Value;\n\
                       g.Dispose();\n\
                       return joined + \" \" + hits.Load() + \" \" + sum + \" \" + scoped.Load() + \" \"\n\
                           + first + second + once.Get() + \" \" + cell.Get();\n\
                   }\n\
               }\n";
    let Some(out) = support::run(&[("p.ys", src)], "t.PScript.P.Run()") else {
        return;
    };
    assert_eq!(out, "4 4000 4000 15 TrueFalsea 7");
}
