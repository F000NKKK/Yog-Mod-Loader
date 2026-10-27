//! Sources that must read cleanly, and ones whose problems must be found.

use yog_ids::script_problem;
use yog_script_analyze::ast::{BinOp, DeclKind, ExprKind, PatchItem, Stmt};
use yog_script_analyze::parse;

/// The complete examples of the grammar (§7): they must read without a problem.
#[test]
fn the_grammars_examples_read() {
    let mut read = 0;
    for (lang, code) in examples() {
        let parsed = parse(&format!("example.{lang}"), &code);
        assert!(
            parsed.problems.is_empty(),
            "{lang} example: {:#?}",
            parsed.problems
        );
        assert!(!parsed.file.decls.is_empty());
        read += 1;
    }
    assert_eq!(read, 2, "a .ys and a .yns example");
}

/// The code blocks of the grammar's §7.
pub fn examples() -> Vec<(String, String)> {
    let doc = include_str!("../../../../docs/design/ys-grammar.md");
    let section = doc
        .split("## 7.")
        .nth(1)
        .and_then(|s| s.split("\n## ").next())
        .unwrap();
    let mut out = Vec::new();
    let mut parts = section.split("```");
    parts.next();
    while let (Some(block), _) = (parts.next(), parts.next()) {
        let (lang, code) = block.split_once('\n').unwrap();
        out.push((lang.trim().to_string(), code.to_string()));
    }
    out
}

#[test]
fn a_small_mod_reads() {
    let parsed = parse("Glassy.ys", include_str!("fixtures/Glassy.ys"));
    assert!(parsed.problems.is_empty(), "{:#?}", parsed.problems);
    assert_eq!(parsed.file.decls.len(), 1);
}

fn codes(name: &str, src: &str) -> Vec<String> {
    parse(name, src)
        .problems
        .into_iter()
        .filter_map(|d| d.code.map(|c| c.to_string()))
        .collect()
}

#[test]
fn a_patch_outside_a_yns_file_is_reported() {
    let src = "patch Player {\n    priority 1;\n}\n";
    assert_eq!(codes("a.ys", src), [script_problem::PATCH_OUTSIDE_NANITES]);
    assert!(codes("a.yns", &format!("targets mc 1.20.1..26.2;\n{src}")).is_empty());
    // A `.yns` says its versions; a `.ys` doesn't.
    assert_eq!(codes("a.yns", src), [script_problem::TARGETS]);
    assert_eq!(
        codes("a.ys", "targets mc >= 1.21;\n"),
        [script_problem::TARGETS]
    );
}

#[test]
fn lexing_problems_are_found_where_they_are() {
    let parsed = parse("a.ys", "const string A = \"open;\nconst i32 B = 1;\n");
    let first = &parsed.problems[0];
    assert_eq!(first.code.as_deref(), Some(script_problem::UNTERMINATED));
    assert_eq!((first.line, first.col), (Some(1), Some(18)));
    assert_eq!(
        codes("a.ys", "const i32 A = @item:;\n"),
        [script_problem::BAD_REFERENCE]
    );
    assert_eq!(
        codes("a.ys", "const i32 A = 0x;\n"),
        [script_problem::BAD_NUMBER]
    );
    assert_eq!(
        codes("a.ys", "const i32 A = 1 ` 2;\n"),
        [script_problem::UNEXPECTED_CHAR, script_problem::EXPECTED]
    );
}

/// A broken statement is reported and the ones after it still read.
#[test]
fn reading_goes_on_after_a_problem() {
    let src = "void A() {\n    let = 1;\n    let b = 2;\n}\ni32 C() => 3;\n";
    let parsed = parse("a.ys", src);
    assert_eq!(parsed.problems.len(), 1, "{:#?}", parsed.problems);
    assert_eq!(parsed.problems[0].line, Some(2));
    assert_eq!(parsed.file.decls.len(), 2);
}

/// `at call X (x)`: the one name is the argument, not a type list.
#[test]
fn an_argument_point_reads_both_ways() {
    for point in [
        "at call Vec3.Add (x) { x * 0.5 }",
        "at call Vec3.Add first (x) { x * 0.5 }",
    ] {
        let src = format!(
            "targets mc 1.20.1..26.2;\npatch LivingEntity {{\n    inside void Knockback() {{\n        {point}\n    }}\n}}\n"
        );
        let parsed = parse("a.yns", &src);
        assert!(
            parsed.problems.is_empty(),
            "{point}: {:#?}",
            parsed.problems
        );
        let DeclKind::Patch(patch) = &parsed.file.decls[1].kind else {
            panic!()
        };
        assert!(matches!(patch.items[0], PatchItem::Inside { .. }));
    }
}

/// Every word has a description editors can show.
#[test]
fn every_word_is_described() {
    for word in yog_script_analyze::words::ALL {
        assert!(!word.doc.is_empty(), "{word:?}");
        assert!(!word.roles.is_empty(), "{word:?}");
    }
}

/// The value of `let v = <src>;` in a method.
fn value(src: &str) -> yog_script_analyze::ast::Expr {
    let parsed = parse("a.ys", &format!("void F() {{ let v = {src}; }}\n"));
    assert!(parsed.problems.is_empty(), "{src}: {:#?}", parsed.problems);
    let DeclKind::Method(m) = parsed.file.decls.into_iter().next().unwrap().kind else {
        panic!()
    };
    let Some(yog_script_analyze::ast::MethodBody::Block(b)) = m.body else {
        panic!()
    };
    let Some(Stmt::Local { value: Some(v), .. }) = b.stmts.into_iter().next() else {
        panic!()
    };
    v
}

/// Operators bind by their levels (`ys-grammar.md` §3.2), words and signs alike.
#[test]
fn operators_bind_by_level() {
    let ExprKind::Binary {
        op: BinOp::Add,
        right,
        ..
    } = value("a + b * c").kind
    else {
        panic!("sum on top")
    };
    assert!(matches!(
        right.kind,
        ExprKind::Binary { op: BinOp::Mul, .. }
    ));
    let ExprKind::Binary {
        op: BinOp::And,
        left,
        ..
    } = value("it is Battery b && b.Charge > 0").kind
    else {
        panic!("`&&` on top")
    };
    assert!(matches!(left.kind, ExprKind::Is { .. }));
    assert!(matches!(
        value("x ?? y ?? z").kind,
        ExprKind::Binary {
            op: BinOp::Coalesce,
            ..
        }
    ));
    assert!(matches!(
        value("x as IChargeable").kind,
        ExprKind::As { .. }
    ));
    assert!(matches!(
        value("a > 1 ? b : c").kind,
        ExprKind::Conditional { .. }
    ));
    assert!(matches!(value("(i32)x").kind, ExprKind::Cast { .. }));
    assert!(matches!(value("(x)").kind, ExprKind::Ident(_)));
    assert!(matches!(value("x => x * 2").kind, ExprKind::Lambda { .. }));
    assert!(matches!(
        value("(a, b) => a + b").kind,
        ExprKind::Lambda { .. }
    ));
    assert!(matches!(
        value("new Pos(1, 2, 3) { X = 4 }").kind,
        ExprKind::New { .. }
    ));
    assert!(matches!(
        value("tier switch { Tier.Wood or Tier.Stone => 1, _ => 2 }").kind,
        ExprKind::Switch { .. }
    ));
    let ExprKind::Call { args, .. } = value("list.Select { it * 2 }").kind else {
        panic!("a trailing lambda")
    };
    assert!(matches!(args[0].value.kind, ExprKind::Lambda { .. }));
    assert!(matches!(
        value("[\"a\" => 1]").kind,
        ExprKind::Collection(_)
    ));
    assert!(matches!(
        value("OfType<Battery>()").kind,
        ExprKind::Call { .. }
    ));
    assert!(matches!(
        value("a < b").kind,
        ExprKind::Binary { op: BinOp::Lt, .. }
    ));
    assert!(matches!(
        value("target!.Health").kind,
        ExprKind::Member { .. }
    ));
}

/// Members written type first tell methods, fields and properties apart.
#[test]
fn members_are_told_by_what_follows_their_name() {
    let src = "class Timer(Ticks every) {\n    Ticks Left = every;\n    public bool Done => Left <= 0t;\n    \
               i32 Count { get; set; } = 0;\n    public Timer() : this(20t) { }\n    \
               List<i32> Ticks<T>(T x) where T : Entity { return []; }\n}\n";
    let parsed = parse("a.ys", src);
    assert!(parsed.problems.is_empty(), "{:#?}", parsed.problems);
    let DeclKind::Type { body, header, .. } = &parsed.file.decls[0].kind else {
        panic!()
    };
    assert!(header.ctor.is_some());
    let kinds: Vec<_> = body
        .iter()
        .map(|d| std::mem::discriminant(&d.kind))
        .collect();
    assert_eq!(kinds.len(), 5);
    assert!(matches!(body[0].kind, DeclKind::Field(_)));
    assert!(matches!(body[1].kind, DeclKind::Property { .. }));
    assert!(matches!(body[2].kind, DeclKind::Property { .. }));
    assert!(matches!(body[3].kind, DeclKind::Ctor(_)));
    assert!(matches!(body[4].kind, DeclKind::Method(_)));
}

#[test]
fn the_type_system_reads() {
    let parsed = parse("Types.ys", include_str!("fixtures/Types.ys"));
    assert!(parsed.problems.is_empty(), "{:#?}", parsed.problems);
    let checked = yog_script_analyze::check("Types.ys", include_str!("fixtures/Types.ys"));
    assert!(checked.problems.is_empty(), "{:#?}", checked.problems);
}

#[test]
fn a_lifetime_is_declared_where_it_is_used() {
    let src = "class A { static in 'b i32 F<'a>(in 'a List<i32> x) => x[0]; }";
    let checked = yog_script_analyze::check("a.ys", src);
    let codes: Vec<_> = checked
        .problems
        .iter()
        .filter_map(|d| d.code.as_deref())
        .collect();
    assert_eq!(codes, [script_problem::LIFETIME]);
}

#[test]
fn a_stray_closer_is_got_past() {
    // `:` isn't an entry's (`=>` is): reported, and reading goes on past
    // the `]` the failed collection leaves, not round it for ever.
    let found = codes(
        "a.ys",
        "void F() {\n    let d = [\"a\": 1];\n}\nvoid G() {}\n",
    );
    assert_eq!(found, [script_problem::EXPECTED, script_problem::EXPECTED]);
}
