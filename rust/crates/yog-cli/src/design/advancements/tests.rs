//! The advancement tab drawn and laid out as the game does.

use super::*;

fn nodes(pairs: &[(&str, Option<&str>)]) -> Vec<Node> {
    pairs
        .iter()
        .map(|(id, parent)| Node {
            id: id.to_string(),
            parent: parent.map(str::to_string),
            icon: None,
            frame: "task".into(),
            title: String::new(),
            description: String::new(),
            obtained: false,
        })
        .collect()
}

fn at(l: &[(String, (f32, f32))], id: &str) -> (f32, f32) {
    l.iter().find(|(i, _)| i == id).expect(id).1
}

#[test]
fn a_parent_is_centred_on_its_children() {
    let l = layout(&nodes(&[
        ("r", None),
        ("a", Some("r")),
        ("b", Some("r")),
        ("c", Some("r")),
    ]));
    assert_eq!(at(&l, "r"), (0.0, 1.0));
    assert_eq!(
        [at(&l, "a"), at(&l, "b"), at(&l, "c")],
        [(1.0, 0.0), (1.0, 1.0), (1.0, 2.0)]
    );
}

#[test]
fn subtrees_keep_apart() {
    let l = layout(&nodes(&[
        ("r", None),
        ("a", Some("r")),
        ("a1", Some("a")),
        ("a2", Some("a")),
        ("b", Some("r")),
    ]));
    assert_eq!(at(&l, "a1"), (2.0, 0.0));
    assert_eq!(at(&l, "a2"), (2.0, 1.0));
    assert_eq!(at(&l, "a"), (1.0, 0.5));
    assert_eq!(at(&l, "b"), (1.0, 1.5));
    assert_eq!(at(&l, "r"), (0.0, 1.0));
}

#[test]
fn a_cycle_is_still_shown() {
    assert_eq!(
        layout(&nodes(&[("a", Some("b")), ("b", Some("a"))])).len(),
        2
    );
}

#[test]
fn lines_break_at_words() {
    let w = |s: &str| s.chars().count() as f32 * 6.0;
    assert_eq!(split_lines("one two three", 48.0, &w), ["one two", "three"]);
    assert_eq!(cut_to_width("abcdef", 18.0, &w), "abc");
}
