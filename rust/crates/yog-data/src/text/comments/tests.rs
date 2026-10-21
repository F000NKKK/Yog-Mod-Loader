use super::with_comments;
use crate::value::Value;

fn s(v: &Value, key: &str) -> Option<String> {
    v.get(key).and_then(Value::as_str).map(str::to_string)
}

#[test]
fn comments_become_notes() {
    let text = "// The ruby block.\n// Shiny.\n{\n  // How hard.\n  hardness: 3,\n  /* Its drop. */\n  drop: { item: \"@item:ruby\" },\n  list: [\n    // First.\n    { a: 1 },\n    // A number.\n    2,\n  ],\n  // Left at the end.\n}";
    let v = with_comments(text).unwrap();
    assert_eq!(
        s(&v, "comment").as_deref(),
        Some("The ruby block.\nShiny.\nA number.\nLeft at the end.")
    );
    assert_eq!(s(&v, "$hardness").as_deref(), Some("How hard."));
    assert_eq!(
        s(v.get("drop").unwrap(), "comment").as_deref(),
        Some("Its drop.")
    );
    let first = &v.get("list").unwrap().as_list().unwrap()[0];
    assert_eq!(s(first, "comment").as_deref(), Some("First."));
    let keys: Vec<&str> = match &v {
        Value::Map(m) => m.keys().collect(),
        _ => unreachable!(),
    };
    assert_eq!(keys, ["comment", "$hardness", "hardness", "drop", "list"]);
}

#[test]
fn text_without_comments_is_its_value() {
    let text = "{ a: \"// not a comment\", b: [1, 2] }";
    assert_eq!(
        with_comments(text).unwrap(),
        super::super::parse_value(text).unwrap()
    );
}
