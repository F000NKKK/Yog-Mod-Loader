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
    let Some(out) = support::run(
        &[("size.ys", size), ("main.ys", main)],
        "t.MainScript.Calc.Run()",
    ) else {
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
    assert_eq!(
        out, "3 -1 7 -1 1",
        "the argument awaited once, for the box there"
    );
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
/// value changed by each in turn (its guard dropped at its block's end),
/// scoped threads joined, once set once.
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
                                   { let mut g = total.Enter(); g.Value = g.Value + 1; }\n\
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
                       let mut sum = 0i64;\n\
                       { let g = total.Enter(); sum = g.Value; }\n\
                       return joined + \" \" + hits.Load() + \" \" + sum + \" \" + scoped.Load() + \" \"\n\
                           + first + second + once.Get() + \" \" + cell.Get();\n\
                   }\n\
               }\n";
    let Some(out) = support::run(&[("p.ys", src)], "t.PScript.P.Run()") else {
        return;
    };
    assert_eq!(out, "4 4000 4000 15 TrueFalsea 7");
}

/// Dropping, as Rust's: a local's `IDisposable` disposed at its block's
/// end, however it ends; one moved on or returned is its new owner's.
#[test]
fn locals_are_dropped_at_their_scopes_end() {
    let src = "public class Res(string name) : IDisposable {\n\
                   public string Name = name;\n\
                   public void Dispose() { D.Log += Name + \";\"; }\n\
               }\n\
               public static class D {\n\
                   public static string Log = \"\";\n\
                   static Res Made() { let r = new Res(\"made\"); return r; }\n\
                   static void Keep(Res r) { }\n\
                   static i32 Early(bool leave) {\n\
                       let a = new Res(\"early\");\n\
                       if (leave) { return 1; }\n\
                       return 2;\n\
                   }\n\
                   public static string Run() {\n\
                       {\n\
                           let a = new Res(\"a\");\n\
                           let b = new Res(\"b\");\n\
                       }\n\
                       let m = Made();\n\
                       Log += \"|\";\n\
                       let c = new Res(\"c\");\n\
                       Keep(move c);\n\
                       Early(true);\n\
                       return Log;\n\
                   }\n\
               }\n";
    let Some(out) = support::run(&[("d.ys", src)], "t.DScript.D.Run()") else {
        return;
    };
    assert_eq!(
        out, "b;a;|early;",
        "the last made dropped first; made, c moved on"
    );
}

/// Iterators, as C#'s: items given one at a time as they're asked for
/// (an endless one taken from), `yield break` ending it, each walk its own.
#[test]
fn iterators_yield_their_items() {
    let src = "public static class I {\n\
                   public static i32 Made = 0;\n\
                   static IEnumerable<i32> Squares(i32 upTo) {\n\
                       for (let mut i = 1; i <= upTo; i += 1) {\n\
                           Made += 1;\n\
                           if (i * i > 50) { yield break; }\n\
                           yield return i * i;\n\
                       }\n\
                   }\n\
                   static IEnumerable<string> Endless() {\n\
                       let mut n = 0;\n\
                       while (true) { n += 1; yield return \"x\" + n; }\n\
                   }\n\
                   public static string Run() {\n\
                       let mut text = \"\";\n\
                       foreach (let s in Squares(100)) { text += s + \",\"; }\n\
                       let squares = Squares(3);\n\
                       let mut twice = 0;\n\
                       foreach (let s in squares) { twice += s; }\n\
                       foreach (let s in squares) { twice += s; }\n\
                       let first = Endless().Take(3).ToList();\n\
                       return text + \" \" + twice + \" \" + first.Count + first[2] + \" \" + Made;\n\
                   }\n\
               }\n";
    let Some(out) = support::run(&[("i.ys", src)], "t.IScript.I.Run()") else {
        return;
    };
    assert_eq!(out, "1,4,9,16,25,36,49, 28 3x3 14");
}

/// `async` iterators and `await foreach`: items awaited between, walked
/// as they come, `yield break` ending them, a `break` leaving the walk.
#[test]
fn async_iterators_and_await_foreach() {
    let src = "using System.Threading.Tasks;\n\
               public static class Q {\n\
                   static async Task<i32> Twice(i32 x) => await Task.FromResult(x * 2);\n\
                   static async IAsyncEnumerable<i32> Doubles(i32 upTo) {\n\
                       for (let mut i = 1; i <= upTo; i += 1) {\n\
                           if (i > 4) { yield break; }\n\
                           yield return await Twice(i);\n\
                       }\n\
                   }\n\
                   static async Task<string> Walk() {\n\
                       let mut text = \"\";\n\
                       await foreach (let d in Doubles(10)) { text += d + \",\"; }\n\
                       await foreach (let d in Doubles(10)) { if (d > 2) { break; } text += \"[\" + d + \"]\"; }\n\
                       return text;\n\
                   }\n\
                   public static string Run() => Walk().Result;\n\
               }\n";
    let Some(out) = support::run(&[("q.ys", src)], "t.QScript.Q.Run()") else {
        return;
    };
    assert_eq!(out, "2,4,6,8,[2]");
}

/// Tasks across threads: work on the pool awaited, waited for, its
/// awaiter's result; `ConfigureAwait(false)`; `async void` run; an
/// `await` going on in the context it started in (a loop of the code's own).
#[test]
fn tasks_across_threads_and_contexts() {
    let src = "using System.Threading;\nusing System.Threading.Tasks;\n\
               public readonly class Loop : SynchronizationContext {\n\
                   public Lock<List<Action>> Queue = new Lock<List<Action>>(new List<Action>());\n\
                   public override void Post(Action work) { let mut g = Queue.Enter(); g.Value.Add(work); }\n\
                   public i32 RunAll() {\n\
                       let mut ran = 0;\n\
                       while (true) {\n\
                           let mut items = new List<Action>();\n\
                           { let mut g = Queue.Enter(); items = g.Value; g.Value = new List<Action>(); }\n\
                           if (items.Count == 0) { return ran; }\n\
                           foreach (let a in items) { a(); ran += 1; }\n\
                       }\n\
                   }\n\
               }\n\
               public static class T {\n\
                   public static i32 Fired = 0;\n\
                   public static bool OnLoop = false;\n\
                   static async void Fire() { Fired += 1; }\n\
                   static async Task<i32> Off() => await Task.Run(() => 20).ConfigureAwait(false) + 1;\n\
                   static async Task<i32> Back(Loop loop) {\n\
                       let v = await Task.Run(() => 5);\n\
                       OnLoop = SynchronizationContext.Current == loop;\n\
                       return v;\n\
                   }\n\
                   public static string Run() {\n\
                       let a = Task.Run(() => 21 * 2);\n\
                       let b = Task.Run(() => 1);\n\
                       Task.WaitAll(a, b);\n\
                       let first = Task.WaitAny(Task.Run(() => 3));\n\
                       Fire();\n\
                       let off = Off().GetAwaiter().GetResult();\n\
                       let loop = new Loop();\n\
                       SynchronizationContext.SetSynchronizationContext(loop);\n\
                       let back = Back(loop);\n\
                       while (!back.IsCompleted) { loop.RunAll(); Thread.Yield(); }\n\
                       SynchronizationContext.SetSynchronizationContext(null);\n\
                       return a.Result + \" \" + b.Result + \" \" + first + \" \" + Fired + \" \" + off + \" \" + back.Result + \" \" + OnLoop;\n\
                   }\n\
               }\n";
    let Some(out) = support::run(&[("t.ys", src)], "t.TScript.T.Run()") else {
        return;
    };
    assert_eq!(out, "42 1 0 1 21 5 True");
}

/// Channels, semaphores, signals, countdowns, concurrent collections,
/// Parallel and AsyncLocal, across the pool's threads.
#[test]
fn channels_and_coordination() {
    let src = "using System.Threading;\nusing System.Threading.Tasks;\nusing System.Threading.Channels;\nusing System.Collections.Concurrent;\n\
               public static class C {\n\
                   public static AsyncLocal<string> Who = new AsyncLocal<string>();\n\
                   static async Task<i32> Sum(ChannelReader<i32> reader) {\n\
                       let mut total = 0;\n\
                       await foreach (let x in reader.ReadAllAsync()) { total += x; }\n\
                       return total;\n\
                   }\n\
                   static async Task<string> Flows() {\n\
                       Who.Value = \"outer\";\n\
                       let seen = await Task.Run(() => Who.Value ?? \"none\");\n\
                       return seen;\n\
                   }\n\
                   public static string Run() {\n\
                       let ch = Channel.CreateBounded<i32>(4);\n\
                       let w = ch.Writer;\n\
                       let producer = Task.Run(() => { for (let mut i = 1; i <= 100; i += 1) { w.Write(i); } w.Complete(); });\n\
                       let sum = Sum(ch.Reader).Result;\n\
                       producer.Wait();\n\
                       let gate = new Semaphore(2);\n\
                       let inside = new Atomic<i32>(0);\n\
                       let most = new Atomic<i32>(0);\n\
                       let counted = new Countdown(8);\n\
                       let words = new ConcurrentDictionary<i32, string>();\n\
                       Parallel.For(0, 8, i => {\n\
                           gate.Wait();\n\
                           let now = inside.Increment();\n\
                           most.Update(m => now > m ? now : m);\n\
                           Thread.Sleep(TimeSpan.FromMilliseconds(5));\n\
                           inside.Decrement();\n\
                           gate.Release();\n\
                           words.GetOrAdd(i % 3, k => \"w\" + k);\n\
                           counted.Signal();\n\
                       });\n\
                       counted.Wait();\n\
                       let go = new Signal();\n\
                       let waiter = Task.Run(() => { go.Wait(); return 7; });\n\
                       go.Set();\n\
                       Who.Value = null;\n\
                       let flows = Flows().Result;\n\
                       return sum + \" \" + (most.Load() <= 2) + \" \" + words.Count + \" \" + waiter.Result + \" \" + flows + \" \" + (Who.Value ?? \"none\");\n\
                   }\n\
               }\n";
    let Some(out) = support::run(&[("c.ys", src)], "t.CScript.C.Run()") else {
        return;
    };
    assert_eq!(out, "5050 True 3 7 outer none");
}

/// `Lazy<T>`, written in Yog Script over `Once<T>`: made once, by whoever asks first.
#[test]
fn lazy_is_made_once() {
    let src = "using System.Threading;\nusing System.Threading.Tasks;\n\
               public static class L {\n\
                   public static Atomic<i32> Made = new Atomic<i32>(0);\n\
                   public static string Run() {\n\
                       let lazy = new Lazy<string>(() => { Made.Increment(); return \"v\"; });\n\
                       let before = lazy.IsValueCreated;\n\
                       Parallel.For(0, 16, i => { let v = lazy.Value; });\n\
                       return before + \" \" + lazy.Value + \" \" + Made.Load() + \" \" + lazy.IsValueCreated;\n\
                   }\n\
               }\n";
    let Some(out) = support::run(&[("l.ys", src)], "t.LScript.L.Run()") else {
        return;
    };
    assert_eq!(out, "False v 1 True");
}

/// `lock (l, ref let n) { … }`: the value changed by threads in turn,
/// written back however the statement ends (an early `return` too).
#[test]
fn lock_statements_hold_and_write_back() {
    let src = "using System.Threading;\nusing System.Threading.Tasks;\n\
               public static class K {\n\
                   static i32 Bump(Lock<i32> l) {\n\
                       lock (l, ref let n) {\n\
                           n += 1;\n\
                           if (n > 0) { return n; }\n\
                       }\n\
                       return -1;\n\
                   }\n\
                   public static string Run() {\n\
                       let l = new Lock<i32>(0);\n\
                       Parallel.For(0, 1000, i => { lock (l, ref let n) { n += 1; } });\n\
                       let after = Bump(l);\n\
                       lock (l) { }\n\
                       let mut seen = 0;\n\
                       lock (l, ref let n) { seen = n; }\n\
                       return after + \" \" + seen;\n\
                   }\n\
               }\n";
    let Some(out) = support::run(&[("k.ys", src)], "t.KScript.K.Run()") else {
        return;
    };
    assert_eq!(out, "1001 1001");
}

/// A constructor's `params`: the arguments past the others packed, an
/// array given for it as it is, none an empty one.
#[test]
fn constructor_params() {
    let src = "public class Bag {\n\
                   public i32 Count;\n\
                   public string Name;\n\
                   public Bag(string name, params i32[] items) { Name = name; Count = items.Length; }\n\
               }\n\
               public static class B {\n\
                   public static string Run() {\n\
                       let a = new Bag(\"a\", 1, 2, 3);\n\
                       let b = new Bag(\"b\");\n\
                       let c = new Bag(\"c\", new i32[] { 4, 5 });\n\
                       return a.Count + \" \" + b.Count + \" \" + c.Count;\n\
                   }\n\
               }\n";
    let Some(out) = support::run(&[("b.ys", src)], "t.BScript.B.Run()") else {
        return;
    };
    assert_eq!(out, "3 0 2");
}

/// A generic type's static fields: one set per list of its arguments, its
/// first values (a type parameter's too) set when the set is first reached.
#[test]
fn statics_per_closed_type() {
    let src = "public class Counter<TItem> {\n\
                   public static i32 Made = 0;\n\
                   public static string Name = typeof(TItem).Name;\n\
                   public static List<TItem> Seen = new List<TItem>();\n\
                   public Counter(TItem item) { Made += 1; Seen.Add(item); }\n\
                   public static i32 Count() => Made;\n\
                   public readonly i32 Mine() => Made;\n\
               }\n\
               public static class S {\n\
                   public static string Run() {\n\
                       let a = new Counter<i32>(1);\n\
                       let b = new Counter<i32>(2);\n\
                       let c = new Counter<string>(\"x\");\n\
                       Counter<bool>.Made = 7;\n\
                       return Counter<i32>.Made + \" \" + Counter<string>.Count() + \" \" + c.Mine() + \" \" + Counter<bool>.Made\n\
                           + \" \" + Counter<string>.Name + \" \" + Counter<i32>.Seen.Count\n\
                           + \" \" + typeof(Counter<i32>).GetField(\"Made\").GetValue(null);\n\
                   }\n\
               }\n";
    let Some(out) = support::run(&[("s.ys", src)], "t.SScript.S.Run()") else {
        return;
    };
    assert_eq!(out, "2 1 1 7 String 2 2");
}

/// `where` constraints: a type parameter's members are its bounds',
/// arguments that don't meet them refused.
#[test]
fn constraints_give_members() {
    let src = "public interface IScore { i32 Score(); }\n\
               public class Card : IScore { public i32 N; public Card(i32 n) { N = n; } public i32 Score() => N; }\n\
               public class Pile<TItem> where TItem : IScore {\n\
                   public List<TItem> Items = new List<TItem>();\n\
                   public i32 Total() { let mut t = 0; foreach (let c in Items) { t += c.Score(); } return t; }\n\
               }\n\
               public static class W {\n\
                   public static TItem Best<TItem>(TItem a, TItem b) where TItem : IScore => a.Score() >= b.Score() ? a : b;\n\
                   public static TItem Made<TItem>() where TItem : new() => new TItem();\n\
                   public static TItem Max<TItem>(TItem a, TItem b) where TItem : IComparable<TItem> => a.CompareTo(b) >= 0 ? a : b;\n\
                   public static TItem Zero<TItem>() => default(TItem);\n\
                   public static string Run() {\n\
                       let mut p = new Pile<Card>();\n\
                       p.Items.Add(new Card(2));\n\
                       p.Items.Add(new Card(5));\n\
                       return p.Total() + \" \" + Best(new Card(3), new Card(4)).N + \" \" + Made<List<i32>>().Count\n\
                           + \" \" + Max(3, 7) + \" \" + Max(\"b\", \"a\") + \" \" + (Zero<i32>() + 1) + \" \" + Made<i32>();\n\
                   }\n\
               }\n";
    let Some(out) = support::run(&[("w.ys", src)], "t.WScript.W.Run()") else {
        return;
    };
    assert_eq!(out, "7 4 0 7 b 1 0");
}

/// `Task.WhenAll` of tasks with results: their results, in order, as an
/// array of their type; `WhenAny` the first done.
#[test]
fn when_all_gives_results() {
    let src = "using System.Threading.Tasks;\n\
               public static class R {\n\
                   static async Task<i32> Twice(i32 n) { await Task.Yield(); return n * 2; }\n\
                   public static string Run() {\n\
                       let all = Task.WhenAll(Twice(1), Twice(2), Twice(3)).Result;\n\
                       let names = Task.WhenAll(Task.FromResult(\"a\"), Task.FromResult(\"b\")).Result;\n\
                       let first = Task.WhenAny(Task.FromResult(9)).Result.Result;\n\
                       return all[0] + all[1] + all[2] + \" \" + names[1] + \" \" + first;\n\
                   }\n\
               }\n";
    let Some(out) = support::run(&[("r.ys", src)], "t.RScript.R.Run()") else {
        return;
    };
    assert_eq!(out, "12 b 9");
}
