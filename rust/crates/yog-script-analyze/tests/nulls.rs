//! Null state: a nullable local known not null where a condition proves it.

use yog_ids::script_problem;
use yog_script_analyze::check;

fn codes(src: &str) -> Vec<String> {
    check("A.ys", src)
        .problems
        .into_iter()
        .filter_map(|d| d.code.map(|c| c.to_string()))
        .collect()
}

#[test]
fn conditions_prove_names_not_null() {
    let src = "i32 A(string? s) { if (s != null) { return s.Length; } return 0; }\n\
               i32 B(string? s) { if (s == null) { return 0; } return s.Length; }\n\
               i32 C(string? s) => s is not null ? s.Length : 0;\n\
               bool D(string? s) => s != null && s.Length > 2;\n\
               bool E(string? s) => s == null || s.Length > 2;\n\
               i32 F(string? s) { if (!(s is null)) { return s.Length; } return 0; }\n\
               i32 G(string? s) { while (s != null) { return s.Length; } return 0; }\n\
               i32 H(string? s) { let mut t = s; t = \"x\"; return t.Length; }\n\
               class Rule { public string? Name; }\n\
               i32 I(Rule r) => r.Name == null ? 0 : r.Name.Length;\n\
               bool J(Rule r) => r.Name != null && r.Name.StartsWith(\"a\");\n";
    assert_eq!(codes(src), Vec::<String>::new());
}

#[test]
fn what_isnt_proven_stays_nullable() {
    let src = "i32 A(string? s) { if (s != null) { } return s.Length; }\n\
               i32 B(string? s) { let mut t = \"x\"; if (t != null) { t = s; return t.Length; } return 0; }\n\
               bool C(string? s) => s != null || s.Length > 2;\n";
    assert_eq!(codes(src), vec![script_problem::NULLABLE; 3]);
}
