//! What crosses threads (`ys-concurrency.md` §2): a lambda's captures
//! where a `[Crosses]` parameter takes it, a value handed over, `unsafe`.

use yog_ids::script_problem;
use yog_script_analyze::check;

fn codes(src: &str) -> Vec<String> {
    check("a.ys", &format!("using System.Threading;\nusing System.Threading.Tasks;\nusing System.Threading.Channels;\n{src}"))
        .problems
        .into_iter()
        .filter_map(|d| d.code.map(|c| c.to_string()))
        .collect()
}

#[test]
fn sendable_captures_cross() {
    let src = "public class Counter { public i32 Hits; }\n\
               public record Name(string Text);\n\
               public static class A {\n\
                   public static void Run() {\n\
                       let n = 5;\n\
                       let c = new Counter();\n\
                       let guarded = new Lock<Counter>(new Counter());\n\
                       let both = new Shared<Name>(new Name(\"x\"));\n\
                       Task.Run(() => n + c.Hits);\n\
                       Thread.Run(() => { guarded.Enter().Value.Hits += 1; return both.Value.Text; });\n\
                       Parallel.For(0, 10, i => { guarded.Enter().Value.Hits += i + n; });\n\
                   }\n\
               }\n";
    assert_eq!(codes(src), Vec::<String>::new());
}

#[test]
fn unsendable_captures_are_errors() {
    // A `Cell` crosses on its own but isn't shared; `Shared` of a changeable type isn't either.
    let src = "public class Cache { public Cell<i32> Hits = new Cell<i32>(0); }\n\
               public class Counter { public i32 Hits; }\n\
               public static class A {\n\
                   public static void Run() {\n\
                       let cache = new Cache();\n\
                       let both = new Shared<Counter>(new Counter());\n\
                       Parallel.For(0, 10, i => { cache.Hits.Set(i); });\n\
                       Task.Run(() => both.Value.Hits);\n\
                   }\n\
               }\n";
    assert_eq!(
        codes(src),
        [script_problem::THREADS, script_problem::THREADS]
    );
}

#[test]
fn this_and_statics_must_be_shareable() {
    let src = "public class Worker {\n\
                   public i32 Done;\n\
                   public static i32 Total;\n\
                   [ThreadStatic] public static i32 Mine;\n\
                   public i32 Step() => Done;\n\
                   public static i32 Helper() => 1;\n\
                   public void Go() {\n\
                       Task.Run(() => Done + 1);\n\
                       Task.Run(() => Step());\n\
                       Task.Run(() => Helper());\n\
                       Task.Run(() => Total + Mine);\n\
                       Task.Run(() => Mine);\n\
                   }\n\
               }\n";
    assert_eq!(
        codes(src),
        [script_problem::THREADS, script_problem::THREADS]
    );
}

#[test]
fn values_handed_over_cross() {
    let src = "public class Counter { public i32 Hits; }\n\
               public static class A {\n\
                   public static void Run(ChannelWriter<Counter> w, ChannelWriter<LockGuard<i32>> g, LockGuard<i32> held) {\n\
                       w.TryWrite(new Counter());\n\
                       g.TryWrite(move held);\n\
                   }\n\
               }\n";
    assert_eq!(codes(src), [script_problem::THREADS]);
}

#[test]
fn unsafe_skips_the_checks_with_a_warning() {
    let src = "public class Counter { public i32 Hits; }\n\
               public static class A {\n\
                   public static void Run() {\n\
                       let mut c = new Counter();\n\
                       unsafe { Parallel.For(0, 10, i => { c.Hits += i; }); }\n\
                   }\n\
               }\n";
    assert_eq!(codes(src), [script_problem::UNSAFE]);
}

#[test]
fn what_a_thread_takes_is_moved_there() {
    let src = "public static class A {\n\
                   public static void Run() {\n\
                       let items = new List<i32>();\n\
                       let n = 3;\n\
                       Task.Run(() => items.Count + n);\n\
                       let again = n + 1;\n\
                       let gone = items.Count;\n\
                   }\n\
               }\n";
    assert_eq!(codes(src), [script_problem::MOVED]);
}
