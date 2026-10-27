//! Patterns run on the JVM: list patterns on arrays, lists and strings.

mod support;

const SRC: &str = r#"using System.Collections.Generic;
public static class P {
    static string Shape(i32[] xs) => xs switch {
        [] => "empty",
        [let one] => "one {one}",
        [0, ..] => "starts at zero",
        [let first, .., let last] when first == last => "same ends",
        [let first, .. let middle, let last] => "{first}+{middle.Length}+{last}",
    };
    static string Words(List<string> ws) {
        if (ws is ["hi", .. let rest]) {
            return "greet " + rest.Count;
        }
        return "no";
    }
    static bool Quoted(string s) => s is ['"', .., '"'];
    public static string Run() => Shape(new i32[0]) + "|" + Shape(new i32[] { 7 }) + "|" + Shape(new i32[] { 0, 5 }) + "|" + Shape(new i32[] { 3, 1, 3 }) + "|" + Shape(new i32[] { 1, 2, 3, 4 }) + "|" + Words(new List<string> { "hi", "a", "b" }) + "|" + Quoted("\"x\"") + Quoted("x");
}
"#;

/// `[]`, `[let one]`, `[0, ..]`, `[a, .., b] when`, `[a, .. let rest, b]`
/// on an array; `["hi", .. let rest]` on a list; `['"', .., '"']` on a string.
#[test]
fn list_patterns() {
    let Some(out) = support::run(&[("P.ys", SRC)], "t.PScript.P.Run()") else {
        return;
    };
    assert_eq!(
        out,
        "empty|one 7|starts at zero|same ends|1+2+4|greet 2|TrueFalse"
    );
}
