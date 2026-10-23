//! Names resolved and misplaced words found (`yog_script::check`).

use yog_ids::script_problem;
use yog_script::check;
use yog_script::resolve::DefKind;

fn codes(name: &str, src: &str) -> Vec<String> {
    check(name, src)
        .problems
        .into_iter()
        .filter_map(|d| d.code.map(|c| c.to_string()))
        .collect()
}

/// The grammar's complete examples (§7) and the small mod resolve without
/// a problem: nothing in them is out of place.
#[test]
fn the_examples_resolve_cleanly() {
    let doc = include_str!("../../../../docs/design/ys-grammar.md");
    let section = doc
        .split("## 7.")
        .nth(1)
        .and_then(|s| s.split("\n## ").next())
        .unwrap();
    let mut parts = section.split("```");
    parts.next();
    while let (Some(block), _) = (parts.next(), parts.next()) {
        let (lang, code) = block.split_once('\n').unwrap();
        let checked = check(&format!("example.{}", lang.trim()), code);
        assert!(
            checked.problems.is_empty(),
            "{lang}: {:#?}",
            checked.problems
        );
    }
    let glassy = check("glassy.ys", include_str!("fixtures/glassy.ys"));
    assert!(glassy.problems.is_empty(), "{:#?}", glassy.problems);
}

#[test]
fn a_name_declared_twice_is_found() {
    let src = "fn charge() = 1\nfn charge() = 2\n";
    assert_eq!(codes("a.ys", src), [script_problem::DUPLICATE]);
    // A block may bind a name again: the later one hides the earlier.
    assert!(codes("a.ys", "fn f() {\n    let x = 1\n    let x = x + 1\n}\n").is_empty());
}

#[test]
fn break_belongs_in_a_loop() {
    assert_eq!(
        codes("a.ys", "fn f() {\n    break\n}\n"),
        [script_problem::OUTSIDE_LOOP]
    );
    assert!(codes("a.ys", "fn f() {\n    loop {\n        break\n    }\n}\n").is_empty());
    // A lambda's body is its own: the loop around it isn't.
    let src = "fn f(xs: List<Int>) {\n    loop {\n        xs.each { break }\n    }\n}\n";
    assert_eq!(codes("a.ys", src), [script_problem::OUTSIDE_LOOP]);
}

#[test]
fn it_and_self_belong_where_they_mean_something() {
    assert_eq!(
        codes("a.ys", "fn f() = it\n"),
        [script_problem::IT_OUTSIDE_LAMBDA]
    );
    assert!(codes("a.ys", "fn f(xs: List<Int>) = xs.map { it * 2 }\n").is_empty());
    assert_eq!(
        codes("a.ys", "fn f() = self\n"),
        [script_problem::SELF_OUTSIDE_MEMBER]
    );
    assert!(
        codes(
            "a.ys",
            "record Pos(x: Int) {\n    fn twice() = self.x * 2\n}\n"
        )
        .is_empty()
    );
}

#[test]
fn only_variables_are_assigned() {
    let src = "fn f() {\n    let a = 1\n    a = 2\n}\n";
    assert_eq!(codes("a.ys", src), [script_problem::ASSIGN_TO_CONSTANT]);
    let src = "fn f(n: Int) {\n    n += 1\n}\n";
    assert_eq!(codes("a.ys", src), [script_problem::ASSIGN_TO_CONSTANT]);
    assert!(codes("a.ys", "fn f() {\n    var a = 1\n    a = 2\n}\n").is_empty());
}

#[test]
fn skip_belongs_in_a_before() {
    let before = "patch Player {\n    before tick() {\n        skip\n    }\n}\n";
    assert!(
        codes("a.yns", before).is_empty(),
        "{:?}",
        codes("a.yns", before)
    );
    let after = "patch Player {\n    after tick() {\n        skip\n    }\n}\n";
    assert_eq!(codes("a.yns", after), [script_problem::SKIP_OUTSIDE_BEFORE]);
}

#[test]
fn a_use_names_its_definition() {
    let src = "fn area(w: Int, h: Int) = w * h\nfn big() = area(10, 20)\n";
    let checked = check("a.ys", src);
    let at = src.find("area(10").unwrap() as u32;
    let def = checked.symbols.def_at(at).unwrap();
    assert_eq!((def.name.as_str(), def.kind), ("area", DefKind::Fn));
    assert_eq!(def.span.start, src.find("area").unwrap() as u32);
}
