//! Checked code run: what it computes, where it panics.

use yog_script_analyze::{check, system};
use yog_script_run::{Interp, Program, Value};

/// `src` checked cleanly, then `method` called with `args`.
fn run(src: &str, method: &str, args: Vec<Value>) -> Result<Value, String> {
    let checked = check("a.ys", src);
    assert!(checked.problems.is_empty(), "{:#?}", checked.problems);
    let program = Program::of(&[&system::DECLS, &checked.file.decls]);
    let mut i = Interp::new(&program);
    i.call(method, args).map_err(|p| p.message)
}

fn int(n: i64) -> Value {
    Value::Int(n)
}

#[test]
fn methods_loops_and_recursion() {
    let src = "i32 Fact(i32 n) => n <= 1 ? 1 : n * Fact(n - 1);\n\
               i32 Sum(i32 n) { let mut total = 0; for (let mut i = 1; i <= n; i += 1) { total += i; } return total; }\n\
               i32 Evens(i32 n) { let mut c = 0; foreach (let i in 0..n) { if (i % 2 == 1) continue; c += 1; } return c; }\n";
    assert_eq!(run(src, "Fact", vec![int(5)]), Ok(int(120)));
    assert_eq!(run(src, "Sum", vec![int(10)]), Ok(int(55)));
    assert_eq!(run(src, "Evens", vec![int(10)]), Ok(int(6)));
}

#[test]
fn objects_inherit_and_structs_copy() {
    let src = "abstract class Machine { public i32 Stored = 0; public abstract i32 Rate(); \
               public void Tick() { Stored += Rate(); } }\n\
               class Battery : Machine { public override i32 Rate() => 4; }\n\
               record struct Pos(i32 X, i32 Y);\n\
               i32 Run() { let mut b = new Battery(); b.Tick(); b.Tick(); return b.Stored; }\n\
               i32 Copy() { let p = new Pos(1, 2); let mut q = p; return q.X + p.Y; }\n";
    assert_eq!(run(src, "Run", vec![]), Ok(int(8)));
    assert_eq!(run(src, "Copy", vec![]), Ok(int(3)));
}

#[test]
fn enums_results_and_patterns() {
    let src = "enum Mode { Idle, Charging(i32 Rate), Full }\n\
               string Label(Mode m) => m switch { Mode.Idle => \"idle\", Mode.Charging(let r) when r > 3 => \"fast\", \
               Mode.Charging(let r) => \"slow\", Mode.Full => \"full\" };\n\
               string Fast() => Label(Mode.Charging(5));\n\
               Result<i32, Error> Parse(i32 x) => x > 0 ? Ok(x) : Err(new Error(\"not positive\"));\n\
               Result<i32, Error> Twice(i32 x) { let v = try Parse(x); return Ok(v * 2); }\n\
               i32 Good() => Twice(4).Unwrap();\n\
               i32 Bad() => Twice(-1).UnwrapOr(7);\n";
    assert_eq!(run(src, "Fast", vec![]), Ok(Value::str("fast")));
    assert_eq!(run(src, "Good", vec![]), Ok(int(8)));
    assert_eq!(run(src, "Bad", vec![]), Ok(int(7)));
}

#[test]
fn panics_stop_with_their_message() {
    let src = "i32 F(i32 x) { assert!(x > 0, \"positive\"); return 10 / x; }\ni32 G() => todo!();\n\
               i32 H(List<i32> xs) => xs[3];\n";
    assert_eq!(run(src, "F", vec![int(0)]), Err("positive".to_string()));
    assert!(run(src, "G", vec![]).is_err());
    assert!(
        run(src, "H", vec![Value::list(vec![int(1)])])
            .unwrap_err()
            .contains("out of range")
    );
}

#[test]
fn lambdas_and_out_parameters() {
    let src = "i32 Apply(Func<i32, i32> f, i32 x) => f(x);\ni32 Twice() => Apply(x => x * 2, 21);\n\
               bool TryHalf(i32 x, out i32 half) { half = x / 2; return x % 2 == 0; }\n\
               i32 Half() { if (TryHalf(10, out let h)) { return h; } return -1; }\n";
    assert_eq!(run(src, "Twice", vec![]), Ok(int(42)));
    assert_eq!(run(src, "Half", vec![]), Ok(int(5)));
}

#[test]
fn test_blocks_pass_or_fail() {
    let src = "i32 Twice(i32 x) => x * 2;\ntest \"twice\" { assert_eq!(Twice(2), 4); }\ntest \"wrong\" { assert_eq!(Twice(2), 5); }\n";
    let checked = check("a.ys", src);
    assert!(checked.problems.is_empty(), "{:#?}", checked.problems);
    let program = Program::of(&[&system::DECLS, &checked.file.decls]);
    let runs = yog_script_run::run_tests(&program, &checked.file.decls);
    let ends: Vec<(&str, bool)> = runs
        .iter()
        .map(|r| (r.name.as_str(), r.outcome.is_ok()))
        .collect();
    assert_eq!(ends, [("twice", true), ("wrong", false)]);
}

/// A `params` parameter takes the arguments past the others, packed.
#[test]
fn params_take_the_rest() {
    let src = "i32 Count(string f, params i32[] xs) => xs.Length;\ni32 Three() => Count(\"x\", 1, 2, 3);\ni32 None() => Count(\"x\");\n";
    assert_eq!(run(src, "Three", vec![]), Ok(int(3)));
    assert_eq!(run(src, "None", vec![]), Ok(int(0)));
}

/// An iterator run at once: its items a list, `yield break` its end.
#[test]
fn iterators_give_their_items() {
    let src = "IEnumerable<i32> Evens(i32 upTo) {\n    for (let mut i = 0; i < upTo; i += 1) {\n        if (i > 6) { yield break; }\n        if (i % 2 == 0) { yield return i; }\n    }\n}\n\
               i32 Sum() { let mut t = 0; foreach (let x in Evens(100)) { t += x; } return t; }\n";
    assert_eq!(run(src, "Sum", vec![]), Ok(int(12)));
}

#[test]
fn collection_initializers_add() {
    let src = "class Bag { public i32 Sum = 0; public void Add(i32 x) { Sum += x; } public void Add(i32 x, i32 times) { Sum += x * times; } }\n\
               i32 Run() { let b = new Bag { 1, 2, { 3, 10 } }; return b.Sum; }\n";
    assert_eq!(run(src, "Run", vec![]), Ok(int(33)));
}

#[test]
fn statics_are_one_per_type() {
    let src = "class Counter { public static i32 Made = 10; public const i32 Step = 2; \
               public Counter() { Made += Step; } public static i32 Total() => Made; }\n\
               i32 Run() { let a = new Counter(); let b = new Counter(); Counter.Made = Counter.Made + 1; return Counter.Total(); }\n";
    assert_eq!(run(src, "Run", vec![]), Ok(int(15)));
}

#[test]
fn async_code_runs_to_its_end() {
    let src = "using System.Threading.Tasks;\n\
               async Task<i32> Twice(i32 n) { await Task.Yield(); return n * 2; }\n\
               async IAsyncEnumerable<i32> Numbers(i32 n) { for (let mut i = 1; i <= n; i += 1) { await Task.Delay(1); yield return i; } }\n\
               async Task<i32> Go() {\n\
                   let mut sum = await Twice(5);\n\
                   await foreach (let x in Numbers(3)) { sum += x; }\n\
                   let all = await Task.WhenAll(Twice(1), Twice(2));\n\
                   let f = async () => 100;\n\
                   return sum + all[1] + await f() + await Task.FromResult(1000);\n\
               }\n\
               i32 Run() => Go().Result;\n";
    assert_eq!(run(src, "Run", vec![]), Ok(int(1120)));
}

#[test]
fn constructors_run_with_their_chain() {
    let src = "class Base { public i32 Log = 0; public Base() { Log = Log * 10 + 1; } public Base(i32 n) { Log = Log * 10 + n; } }\n\
               class Mid : Base { public Mid() : base(2) { Log = Log * 10 + 3; } public Mid(i32 n) : this() { Log = Log * 10 + n; } }\n\
               class Leaf : Base { public Leaf() { Log = Log * 10 + 7; } }\n\
               i32 Run() { let a = new Mid(4); let b = new Leaf(); return a.Log * 100 + b.Log; }\n";
    assert_eq!(run(src, "Run", vec![]), Ok(int(234 * 100 + 17)));
}

#[test]
fn local_functions_run_in_their_block() {
    let src = "i32 Run() { let mut total = 0; void Add(i32 x) { total += x; } Add(2); Add(3); return total + Fact(4); i32 Fact(i32 k) => k <= 1 ? 1 : k * Fact(k - 1); }\n";
    assert_eq!(run(src, "Run", vec![]), Ok(int(29)));
}

#[test]
fn switch_statements_pick_a_section() {
    let src = "i32 Run() { let mut t = 0; foreach (let i in 0..4) { switch (i) { case 0: continue; case 1: case 2: t += 10; break; default: t += i; break; } } return t; }\n";
    assert_eq!(run(src, "Run", vec![]), Ok(int(27)));
}

#[test]
fn ends_and_slices() {
    let src = "i32 Run() { let xs = [1, 2, 3, 4]; let part = xs[1..2]; return xs[^1] * 100 + part[0] * 10 + part.Count; }\n";
    assert_eq!(run(src, "Run", vec![]), Ok(int(422)));
}

#[test]
fn properties_run_their_accessors() {
    let src = "class C { public i32 X { get => field; set => field = value * 2; } \
               public i32 Z { get; set => field = Math.Max(value, 0); } = 5; \
               private i32 _y; public i32 Y { get => _y + 1; set => _y = value; } \
               public i32 Sum => X + Y + Z; }\n\
               i32 Run() { let mut c = new C(); let first = c.Z; c.X = 3; c.Y = 4; c.Z = -7; return first * 1000 + c.Sum * 10 + c.Z; }\n";
    assert_eq!(run(src, "Run", vec![]), Ok(int(5110)));
}

/// `goto` to a label before or after it, out of loops, past a block's
/// locals, beside a loop's `continue` and `break`; `goto case`/`default`;
/// `@"…"`.
#[test]
fn gotos_jump() {
    let src = r#"i32 Retry() { let mut tries = 0; again: tries += 1; if (tries < 3) goto again; return tries; }
string Find() { let mut found = "none"; for (let mut i = 0; i < 5; i += 1) { for (let mut j = 0; j < 5; j += 1) { if (i * j == 6) { found = "{i},{j}"; goto done; } } } done: return found; }
i32 Hoisted() { let mut n = 0; top: let k = n * 2; n += 1; if (n < 4) goto top; return k + n; }
string Outer() { let mut s = ""; foreach (let i in 0..5) { mark: if (i == 1) continue; if (i == 3) break; s = s + i; } return s; }
string Steps(i32 n) { let mut s = ""; switch (n) { case 1: s = s + "one "; goto case 2; case 2: s = s + "two "; goto default; default: s = s + "end"; break; } return s; }
string Run() => Retry() + " " + Find() + " " + Hoisted() + " " + Outer() + " " + Steps(1) + "|" + Steps(5) + "|" + @"C:\m\""q""";
"#;
    assert_eq!(
        run(src, "Run", vec![]),
        Ok(Value::str(r#"3 2,3 10 02 one two end|end|C:\m\"q""#))
    );
}

/// List patterns: lengths, items from both ends, `.. let rest` a slice.
#[test]
fn list_patterns_match_items() {
    let src = r#"string Shape(i32[] xs) => xs switch { [] => "empty", [let one] => "one {one}", [0, ..] => "zero", [let a, .. let mid, let z] => "{a}+{mid.Length}+{z}" };
string Run() => Shape(new i32[0]) + "|" + Shape(new i32[] { 7 }) + "|" + Shape(new i32[] { 0, 5 }) + "|" + Shape(new i32[] { 1, 2, 3, 4 }) + "|" + ("ab" is ['a', ..]);
"#;
    assert_eq!(run(src, "Run", vec![]), Ok(Value::str("empty|one 7|zero|1+2+4|True")));
}
