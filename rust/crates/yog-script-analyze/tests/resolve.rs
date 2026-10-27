//! Names resolved and misplaced words found (`yog_script_analyze::check`).

use yog_ids::script_problem;
use yog_script_analyze::check;
use yog_script_analyze::resolve::DefKind;

/// The type checker's problems.
const TYPES: [&str; 7] = [
    script_problem::TYPE_MISMATCH,
    script_problem::UNKNOWN_TYPE,
    script_problem::NULLABLE,
    script_problem::UNKNOWN_MEMBER,
    script_problem::WRONG_SIDE,
    script_problem::WRONG_ARGS,
    script_problem::UNKNOWN_NAME,
];

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
        // The examples also show parts of Yog's surface still to come
        // (`Level`, blocks as objects, opening screens): what the type
        // checker says of those isn't the resolver's to answer for.
        let checked = check(&format!("example.{}", lang.trim()), code);
        let problems: Vec<_> = checked
            .problems
            .into_iter()
            .filter(|d| !d.code.map(yog_data::Code::as_str).is_some_and(|c| TYPES.contains(&c)))
            .collect();
        assert!(problems.is_empty(), "{lang}: {problems:#?}");
    }
    let glassy = check("Glassy.ys", include_str!("fixtures/Glassy.ys"));
    assert!(glassy.problems.is_empty(), "{:#?}", glassy.problems);
}

#[test]
fn a_name_declared_twice_is_found() {
    let src = "i32 Charge() => 1;\ni32 Charge() => 2;\n";
    assert_eq!(codes("A.ys", src), [script_problem::DUPLICATE]);
    // A block may bind a name again: the later one hides the earlier.
    assert!(
        codes(
            "A.ys",
            "void F() {\n    let x = 1;\n    let x = x + 1;\n}\n"
        )
        .is_empty()
    );
}

#[test]
fn break_belongs_in_a_loop() {
    assert_eq!(
        codes("A.ys", "void F() {\n    break;\n}\n"),
        [script_problem::OUTSIDE_LOOP]
    );
    assert!(
        codes(
            "A.ys",
            "void F() {\n    while (true) {\n        break;\n    }\n}\n"
        )
        .is_empty()
    );
    // A lambda's body is its own: the loop around it isn't.
    let src =
        "void F(List<i32> xs) {\n    while (true) {\n        xs.ForEach { break; };\n    }\n}\n";
    assert_eq!(codes("A.ys", src), [script_problem::OUTSIDE_LOOP]);
}

#[test]
fn it_and_this_belong_where_they_mean_something() {
    assert_eq!(
        codes("A.ys", "i32 F() => it;\n"),
        [script_problem::IT_OUTSIDE_LAMBDA]
    );
    assert!(
        codes(
            "A.ys",
            "bool F(List<i32> xs) => xs.Exists { it * 2 > 3 };\n"
        )
        .is_empty()
    );
    assert_eq!(
        codes("A.ys", "i32 F() => this;\n"),
        [script_problem::THIS_OUTSIDE_MEMBER]
    );
    assert!(
        codes(
            "A.ys",
            "record Pos(i32 X) {\n    i32 Twice() => this.X * 2;\n}\n"
        )
        .is_empty()
    );
}

#[test]
fn only_what_can_change_is_assigned() {
    let src = "const i32 Max = 3;\nvoid F() {\n    Max = 2;\n}\n";
    assert_eq!(codes("A.ys", src), [script_problem::ASSIGN_TO_CONSTANT]);
    let src = "void F(List<i32> xs) {\n    foreach (let x in xs) x = 1;\n}\n";
    assert_eq!(codes("A.ys", src), [script_problem::ASSIGN_TO_CONSTANT]);
    assert!(
        codes(
            "A.ys",
            "void F(i32 n) {\n    let mut a = 1;\n    a = 2;\n    n += 1;\n}\n"
        )
        .is_empty()
    );
    // `let` is never assigned again; `ref` lends only a `let mut`.
    let src = "void F() {\n    let a = 1;\n    a = 2;\n}\n";
    assert_eq!(codes("A.ys", src), [script_problem::ASSIGN_TO_CONSTANT]);
    let src = "void G(ref i32 x) {}\nvoid F() {\n    let a = 1;\n    G(ref a);\n}\n";
    assert_eq!(codes("A.ys", src), [script_problem::ASSIGN_TO_CONSTANT]);
}

#[test]
fn skip_belongs_in_a_before() {
    let before = "targets mc 1.20.1..26.2;\npatch Player {\n    before void Tick() {\n        skip;\n    }\n}\n";
    assert!(
        codes("a.yns", before).is_empty(),
        "{:?}",
        codes("a.yns", before)
    );
    let after = "targets mc 1.20.1..26.2;\npatch Player {\n    after void Tick() {\n        skip;\n    }\n}\n";
    assert_eq!(codes("a.yns", after), [script_problem::SKIP_OUTSIDE_BEFORE]);
}

#[test]
fn await_belongs_in_async() {
    let src = "using System.Threading.Tasks;\nvoid Arm() {\n    await Task.Delay(5s);\n}\n";
    assert_eq!(codes("A.ys", src), [script_problem::AWAIT_OUTSIDE_ASYNC]);
    assert!(
        codes(
            "A.ys",
            "using System.Threading.Tasks;\nasync void Arm() {\n    await Task.Delay(5s);\n}\n"
        )
        .is_empty()
    );
}

/// Names in C#'s case, warned otherwise.
#[test]
fn names_follow_csharps_style() {
    let src = "i32 area(i32 W, i32 h) => W * h;\ninterface Chargeable { }\nclass Box<Item> { }\n";
    assert_eq!(
        codes("A.ys", src),
        [
            script_problem::NAMING,
            script_problem::NAMING,
            script_problem::NAMING,
            script_problem::NAMING
        ]
    );
    let fine = "i32 Area(i32 w, i32 h) => w * h;\ninterface IChargeable { }\nclass Box<TItem> { }\n\
                record Pos(i32 X, i32 Y);\nvoid F() {\n    let _ = 1;\n    let chargeLevel = 2;\n}\n";
    assert!(codes("A.ys", fine).is_empty(), "{:?}", codes("A.ys", fine));
}

#[test]
fn a_use_names_its_definition() {
    let src = "i32 Area(i32 w, i32 h) => w * h;\ni32 Big() => Area(10, 20);\n";
    let checked = check("A.ys", src);
    let at = src.find("Area(10").unwrap() as u32;
    let def = checked.symbols.def_at(at).unwrap();
    assert_eq!((def.name.as_str(), def.kind), ("Area", DefKind::Method));
    assert_eq!(def.span.start, src.find("Area").unwrap() as u32);
}

/// `await` waits in an `async` method or lambda only, as C#'s: a lambda
/// inside an `async` method isn't one.
#[test]
fn await_needs_an_async_lambda_too() {
    let src = "using System.Threading.Tasks;\nasync Task F() {\n    Action a = () => {\n        await Task.Yield();\n    };\n}\n";
    let codes: Vec<_> = yog_script_analyze::check("A.ys", src)
        .problems
        .into_iter()
        .filter_map(|d| d.code)
        .collect();
    assert!(
        codes
            .iter()
            .any(|c| c.as_str() == yog_ids::script_problem::AWAIT_OUTSIDE_ASYNC),
        "{codes:?}"
    );
}

/// `goto` reaches a label of its block or one around it, never one inside
/// another block nor out of a lambda; a label once; `goto case` in a `switch`.
#[test]
fn gotos_reach_their_labels() {
    let fine = "void F(i32 n) {\n    start:\n    if (n > 0) { goto start; }\n    switch (n) { case 1: goto case 2; case 2: goto default; default: break; }\n}\n";
    assert!(codes("A.ys", fine).is_empty(), "{:?}", codes("A.ys", fine));
    let inside = "void F() {\n    goto deep;\n    { deep: return; }\n}\n";
    assert_eq!(codes("A.ys", inside), [script_problem::GOTO]);
    let twice = "void F() {\n    a: ;\n    { a: ; }\n}\n";
    assert_eq!(codes("A.ys", twice), [script_problem::GOTO]);
    let no_switch = "void F() {\n    goto default;\n}\n";
    assert_eq!(codes("A.ys", no_switch), [script_problem::GOTO]);
    let lambda = "void F() {\n    outside:\n    Action a = () => { goto outside; };\n}\n";
    assert_eq!(codes("A.ys", lambda), [script_problem::GOTO]);
}
