//! The System library: its own sources check, and code uses it.

use yog_ids::script_problem;
use yog_script_analyze::ast::DeclKind;
use yog_script_analyze::{check, system};

fn codes(src: &str) -> Vec<String> {
    check("a.ys", src)
        .problems
        .into_iter()
        .filter_map(|d| d.code.map(|c| c.to_string()))
        .collect()
}

#[test]
fn the_librarys_sources_check() {
    assert!(!system::sources().is_empty());
    for (path, src) in system::sources() {
        let checked = check(path, src);
        assert!(
            checked.problems.is_empty(),
            "{path}: {:#?}",
            checked.problems
        );
        // As .NET's IDE0130: a file's namespace is its folders.
        let folders = path.rsplit_once(system::FOLDER_SEP).map_or("", |(f, _)| f);
        let declared = checked.file.decls.iter().find_map(|d| match &d.kind {
            DeclKind::Namespace { path, .. } => Some(
                path.iter()
                    .map(|n| n.text.as_str())
                    .collect::<Vec<_>>()
                    .join("."),
            ),
            _ => None,
        });
        assert_eq!(
            declared.as_deref(),
            Some(folders.replace(system::FOLDER_SEP, ".").as_str()),
            "{path}"
        );
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

#[test]
fn time_spans_add_and_compare_by_their_operators() {
    let ok = "TimeSpan Wait() => TimeSpan.FromSeconds(2) + TimeSpan.FromMilliseconds(500);\n\
              bool Long(TimeSpan t) => t * 2.0 >= TimeSpan.FromMinutes(1) && -t < TimeSpan.Zero;\n\
              f64 Halves(TimeSpan t) => t / TimeSpan.FromSeconds(0.5);\n";
    assert!(codes(ok).is_empty(), "{:?}", codes(ok));
    // Two spans don't multiply; a span isn't a number.
    let bad = "TimeSpan F(TimeSpan t) => t * t;\nf64 G(TimeSpan t) => t;\n";
    assert_eq!(
        codes(bad),
        [script_problem::TYPE_MISMATCH, script_problem::TYPE_MISMATCH]
    );
}

#[test]
fn vectors_are_numerics_by_their_operators() {
    let ok = "using System.Numerics;\n\
              Vector3 Step(Vector3 from, Vector3 to, f32 speed) => from + Vector3.Normalize(to - from) * speed;\n\
              bool Still(Vector2 v) => -v == Vector2.Zero;\n\
              f32 Far(Vector3 a, Vector3 b) => Math.Max(Vector3.Distance(a, b), 1);\n";
    assert!(codes(ok).is_empty(), "{:?}", codes(ok));
    // A vector isn't a number; `System.Numerics` is `using`'d.
    let bad = "using System.Numerics;\nf32 F(Vector3 v) => v + 1;\n";
    assert_eq!(codes(bad), [script_problem::TYPE_MISMATCH]);
    assert_eq!(
        codes("Vector3 F() => default;\n"),
        [script_problem::UNKNOWN_TYPE]
    );
    // Written in full, without the `using`: its members' types are read in
    // its own namespace, not the caller's.
    let full = "System.Numerics.Vector3 F(System.Numerics.Vector3 v) =>\n\
                System.Numerics.Vector3.Normalize(v) + v;\n";
    assert!(codes(full).is_empty(), "{:?}", codes(full));
}
