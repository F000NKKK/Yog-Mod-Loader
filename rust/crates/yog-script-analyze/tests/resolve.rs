//! Names resolved and misplaced words found (`yog_script_analyze::check`).

use yog_ids::script_problem;
use yog_script_analyze::check;
use yog_script_analyze::resolve::DefKind;

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
    let src = "int Charge() => 1;\nint Charge() => 2;\n";
    assert_eq!(codes("a.ys", src), [script_problem::DUPLICATE]);
    // A block may bind a name again: the later one hides the earlier.
    assert!(
        codes(
            "a.ys",
            "void F() {\n    var x = 1;\n    var x = x + 1;\n}\n"
        )
        .is_empty()
    );
}

#[test]
fn break_belongs_in_a_loop() {
    assert_eq!(
        codes("a.ys", "void F() {\n    break;\n}\n"),
        [script_problem::OUTSIDE_LOOP]
    );
    assert!(
        codes(
            "a.ys",
            "void F() {\n    while (true) {\n        break;\n    }\n}\n"
        )
        .is_empty()
    );
    // A lambda's body is its own: the loop around it isn't.
    let src =
        "void F(List<int> xs) {\n    while (true) {\n        xs.ForEach { break; };\n    }\n}\n";
    assert_eq!(codes("a.ys", src), [script_problem::OUTSIDE_LOOP]);
}

#[test]
fn it_and_this_belong_where_they_mean_something() {
    assert_eq!(
        codes("a.ys", "int F() => it;\n"),
        [script_problem::IT_OUTSIDE_LAMBDA]
    );
    assert!(
        codes(
            "a.ys",
            "List<int> F(List<int> xs) => xs.Select { it * 2 };\n"
        )
        .is_empty()
    );
    assert_eq!(
        codes("a.ys", "int F() => this;\n"),
        [script_problem::THIS_OUTSIDE_MEMBER]
    );
    assert!(
        codes(
            "a.ys",
            "record Pos(int X) {\n    int Twice() => this.X * 2;\n}\n"
        )
        .is_empty()
    );
}

#[test]
fn only_what_can_change_is_assigned() {
    let src = "const int Max = 3;\nvoid F() {\n    Max = 2;\n}\n";
    assert_eq!(codes("a.ys", src), [script_problem::ASSIGN_TO_CONSTANT]);
    let src = "void F(List<int> xs) {\n    foreach (var x in xs) x = 1;\n}\n";
    assert_eq!(codes("a.ys", src), [script_problem::ASSIGN_TO_CONSTANT]);
    assert!(
        codes(
            "a.ys",
            "void F(int n) {\n    var a = 1;\n    a = 2;\n    n += 1;\n}\n"
        )
        .is_empty()
    );
}

#[test]
fn skip_belongs_in_a_before() {
    let before = "patch Player {\n    before void Tick() {\n        skip;\n    }\n}\n";
    assert!(
        codes("a.yns", before).is_empty(),
        "{:?}",
        codes("a.yns", before)
    );
    let after = "patch Player {\n    after void Tick() {\n        skip;\n    }\n}\n";
    assert_eq!(codes("a.yns", after), [script_problem::SKIP_OUTSIDE_BEFORE]);
}

#[test]
fn await_belongs_in_async() {
    let src = "void Arm() {\n    await Task.Delay(5s);\n}\n";
    assert_eq!(codes("a.ys", src), [script_problem::AWAIT_OUTSIDE_ASYNC]);
    assert!(codes("a.ys", "async void Arm() {\n    await Task.Delay(5s);\n}\n").is_empty());
}

/// Names in C#'s case, warned otherwise.
#[test]
fn names_follow_csharps_style() {
    let src = "int area(int W, int h) => W * h;\ninterface Chargeable { }\nclass Box<Item> { }\n";
    assert_eq!(
        codes("a.ys", src),
        [
            script_problem::NAMING,
            script_problem::NAMING,
            script_problem::NAMING,
            script_problem::NAMING
        ]
    );
    let fine = "int Area(int w, int h) => w * h;\ninterface IChargeable { }\nclass Box<TItem> { }\n\
                record Pos(int X, int Y);\nvoid F() {\n    var _ = 1;\n    var chargeLevel = 2;\n}\n";
    assert!(codes("a.ys", fine).is_empty(), "{:?}", codes("a.ys", fine));
}

#[test]
fn a_use_names_its_definition() {
    let src = "int Area(int w, int h) => w * h;\nint Big() => Area(10, 20);\n";
    let checked = check("a.ys", src);
    let at = src.find("Area(10").unwrap() as u32;
    let def = checked.symbols.def_at(at).unwrap();
    assert_eq!((def.name.as_str(), def.kind), ("Area", DefKind::Method));
    assert_eq!(def.span.start, src.find("Area").unwrap() as u32);
}
