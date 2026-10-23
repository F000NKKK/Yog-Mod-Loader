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
