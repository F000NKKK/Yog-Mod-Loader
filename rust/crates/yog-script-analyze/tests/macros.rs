//! Declarative macros: rules, fragments, repetitions, hygiene.

use yog_ids::script_problem;
use yog_script_analyze::check;

fn codes(src: &str) -> Vec<String> {
    check("A.ys", src)
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

#[test]
fn the_built_in_macros_are_typed() {
    let ok = "i32 F(i32 x) { assert!(x > 0, \"positive\"); assert_eq!(x, 3); let y = dbg!(x * 2); \
              if (y > 9) { panic!(\"too big\"); } return y; }\ni32 G() => todo!();\n";
    assert!(codes(ok).is_empty(), "{:?}", codes(ok));
    // `assert!` wants a bool; `dbg!` gives its argument's type; an unknown macro.
    assert_eq!(
        codes("void F() { assert!(3); string s = dbg!(1); nope!(1); }\n"),
        [
            script_problem::TYPE_MISMATCH,
            script_problem::TYPE_MISMATCH,
            script_problem::MACRO
        ]
    );
    // `x!` alone still says `x` isn't null.
    assert!(codes("i32 F(i32? x) => x!;\n").is_empty());
}

/// The files a test's source includes: one, `help.txt`.
struct OneFile;

impl yog_script_analyze::macros::ModFiles for OneFile {
    fn read(&self, _source: &str, path: &str) -> Result<String, String> {
        match path {
            "help.txt" => Ok("say {hi}".to_string()),
            _ => Err("no such file".to_string()),
        }
    }
}

/// `include_str!`: the file's text, written as it is; one that can't be read reported.
#[test]
fn include_str_is_the_files_text() {
    let env = yog_script_analyze::Env {
        files: Some(&OneFile),
        ..Default::default()
    };
    let src = "string Help() => include_str!(\"help.txt\");\n";
    let checked = yog_script_analyze::check_in("A.ys", src, env);
    assert!(checked.problems.is_empty(), "{:?}", checked.problems);
    let text = format!("{:?}", checked.file.decls);
    assert!(text.contains("say {hi}"), "{text}");
    let missing = "string Help() => include_str!(\"gone.txt\");\n";
    let codes: Vec<_> = yog_script_analyze::check_in("A.ys", missing, env)
        .problems
        .into_iter()
        .filter_map(|d| d.code)
        .collect();
    assert_eq!(codes, [script_problem::MACRO]);
}
