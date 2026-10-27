//! The borrow checker: moves, borrows and paths, as Rust's.

use yog_ids::script_problem;
use yog_script_analyze::check;

fn codes(body: &str) -> Vec<String> {
    let src = format!(
        "class Box {{}}\nclass A {{\n    static void Keep(move Box b) {{}}\n    static void Look(in Box b) {{}}\n    \
         static void Fill(ref Box b) {{}}\n    static void Twice(ref Box a, in Box b) {{}}\n    {body}\n}}\n"
    );
    check("A.ys", &src)
        .problems
        .into_iter()
        .filter_map(|d| d.code.map(|c| c.to_string()))
        .collect()
}

#[test]
fn a_moved_value_is_gone() {
    assert!(codes("static void F() { let b = new Box(); Keep(move b); }").is_empty());
    assert_eq!(
        codes("static void F() { let b = new Box(); Keep(move b); Look(b); }"),
        [script_problem::MOVED]
    );
    assert_eq!(
        codes("static void F() { let b = new Box(); let c = move b; Look(b); }"),
        [script_problem::MOVED]
    );
    // Given a value again, it's usable again.
    assert!(
        codes("static void F() { let mut b = new Box(); Keep(move b); b = new Box(); Look(b); }")
            .is_empty()
    );
}

#[test]
fn paths_join_and_loops_turn() {
    // Moved on one branch: maybe moved after it.
    assert_eq!(
        codes("static void F(bool x) { let b = new Box(); if (x) { Keep(move b); } Look(b); }"),
        [script_problem::MOVED]
    );
    // A branch that returns brings nothing to what follows.
    assert!(
        codes(
            "static void F(bool x) { let b = new Box(); if (x) { Keep(move b); return; } Look(b); }"
        )
        .is_empty()
    );
    // What one turn moves, the next can't.
    assert_eq!(
        codes("static void F(bool x) { let b = new Box(); while (x) { Keep(move b); } }"),
        [script_problem::MOVED]
    );
}

#[test]
fn borrows_follow_the_rules() {
    assert_eq!(
        codes("static void F() { let mut b = new Box(); Twice(ref b, in b); }"),
        [script_problem::BORROW]
    );
    // A lent parameter isn't the method's to move.
    assert_eq!(
        codes("static void F(in Box b) { Keep(move b); }"),
        [script_problem::BORROW]
    );
    assert!(codes("static void F(move Box b) { Keep(move b); }").is_empty());
}

#[test]
fn both_ends_pass_alike() {
    assert_eq!(
        codes("static void F() { let b = new Box(); Keep(b); }"),
        [script_problem::PASS]
    );
    assert_eq!(
        codes("static void F() { let mut b = new Box(); Fill(b); }"),
        [script_problem::PASS]
    );
    // An `in` is lent with or without the word.
    assert!(codes("static void F() { let b = new Box(); Look(b); Look(in b); }").is_empty());
}

/// A local is read only once it has a value on every path; an `out`
/// parameter has one on every path out of its method.
#[test]
fn values_are_given_before_use() {
    assert_eq!(
        codes("static i32 F(bool c) { i32 x; if (c) { x = 1; } return x; }"),
        [script_problem::UNASSIGNED]
    );
    assert!(
        codes("static i32 F(bool c) { i32 x; if (c) { x = 1; } else { x = 2; } return x; }")
            .is_empty()
    );
    assert_eq!(
        codes("static bool F(bool c, out i32 x) { if (c) { x = 1; } return c; }"),
        [script_problem::UNASSIGNED]
    );
    assert!(
        codes(
            "static bool F(bool c, out i32 x) { if (c) { x = 1; return true; } x = 0; return c; }"
        )
        .is_empty()
    );
    // Read before it's given, an `out` one is as a local without a value.
    assert_eq!(
        codes("static void F(out i32 x) { let y = x; x = y; }"),
        [script_problem::UNASSIGNED]
    );
    // Given by a call's `out`.
    assert!(
        codes(
            "static void G(out i32 x) { x = 1; }\n    static i32 F() { i32 x; G(out x); return x; }"
        )
        .is_empty()
    );
}
