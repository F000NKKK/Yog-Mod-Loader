//! Declarative macros: rules, fragments, repetitions, hygiene.

use yog_ids::script_problem;
use yog_script_analyze::check;

fn codes(src: &str) -> Vec<String> {
    check("a.ys", src)
        .problems
        .into_iter()
        .filter_map(|d| d.code.map(|c| c.to_string()))
        .collect()
}

const MACROS: &str = "macro Square {\n    ($x:expr) => { $x * $x };\n}\n\
    macro MaxOf {\n    ($x:expr) => { $x };\n    ($x:expr, $($rest:expr),+) => { Math.Max($x, MaxOf!($($rest),+)) };\n}\n\
    macro Swap {\n    ($a:ident, $b:ident) => { let t = $a; $a = $b; $b = t; };\n}\n";

#[test]
fn macros_expand_before_types() {
    // A fragment stays whole (`Square!(1 + 2)` is 9, a number), rules recurse.
    let ok = format!(
        "{MACROS}i32 F() => Square!(1 + 2);\nvoid G() {{ let mut t = 1; let mut u = 2; Swap!(t, u); }}\n"
    );
    assert!(codes(&ok).is_empty(), "{:?}", codes(&ok));
    // What a macro writes is checked as code.
    assert_eq!(
        codes(&format!("{MACROS}string F() => Square!(3);\n")),
        [script_problem::TYPE_MISMATCH]
    );
}

#[test]
fn a_call_no_rule_matches_is_reported() {
    assert_eq!(
        codes(&format!("{MACROS}void G() {{ Swap!(1, 2); }}\n")),
        [script_problem::MACRO]
    );
    assert_eq!(
        codes("macro Bad {\n    ($x) => { $x };\n}\n"),
        [script_problem::MACRO]
    );
}
