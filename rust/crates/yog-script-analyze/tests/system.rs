//! The System library: its own sources check, and code uses it.

use yog_ids::script_problem;
use yog_script_analyze::check;

fn codes(src: &str) -> Vec<String> {
    check("a.ys", src)
        .problems
        .into_iter()
        .filter_map(|d| d.code.map(|c| c.to_string()))
        .collect()
}

#[test]
fn the_librarys_sources_check() {
    for (name, src) in [
        ("System.ys", include_str!("../src/system/System.ys")),
        (
            "System.Meta.ys",
            include_str!("../src/system/System.Meta.ys"),
        ),
        (
            "System.Yog.Native.ys",
            include_str!("../src/system/System.Yog.Native.ys"),
        ),
    ] {
        let problems = check(name, src).problems;
        assert!(problems.is_empty(), "{name}: {problems:#?}");
    }
}

#[test]
fn results_are_rusts() {
    let ok = "Result<i32, Error> Parse(string s) => Ok(4);\n\
              Result<i32, Error> Twice(string s) { let v = try Parse(s); return Ok(v * 2); }\n\
              i32 Max() => Math.Max(1, 2);\n";
    assert!(codes(ok).is_empty(), "{:?}", codes(ok));
    // `try` gives the value's type; a case's data is checked.
    let bad = "Result<i32, Error> Parse(string s) => Ok(\"x\");\n\
               string Twice(string s) { string v = try Parse(s); return v; }\n";
    assert_eq!(
        codes(bad),
        [script_problem::TYPE_MISMATCH, script_problem::TYPE_MISMATCH]
    );
}

#[test]
fn attributes_are_attribute_classes() {
    let ok = "[Obsolete(\"use G\")]\nvoid F() {}\nclass Range(i32 min, i32 max) : Attribute {}\n[Range(0, 10)]\nvoid H() {}\n[Derive(Clone)]\nclass Box {}\n";
    assert!(codes(ok).is_empty(), "{:?}", codes(ok));
    let bad = "class NotOne {}\n[NotOne]\nvoid F() {}\n[Obsolete(3)]\nvoid G() {}\n";
    assert_eq!(
        codes(bad),
        [script_problem::ATTRIBUTE, script_problem::TYPE_MISMATCH]
    );
}

#[test]
fn rust_is_reached_on_the_bus() {
    let ok = "using System.Yog.Native;\n[Service(\"charge:simulate\")]\nstatic extern i64 Simulate(i64 nodes);\ni64 F() => Simulate(3);\n\
              [Provide(\"charge:double\")]\nstatic i64 Double(i64 x) => x * 2;\n";
    assert!(codes(ok).is_empty(), "{:?}", codes(ok));
    assert_eq!(
        codes("static extern i64 Simulate(i64 nodes);\n"),
        [script_problem::ATTRIBUTE]
    );
}
