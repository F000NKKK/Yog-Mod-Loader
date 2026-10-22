//! Sources that must read cleanly, and ones whose problems must be found.

use yog_ids::script_problem;
use yog_script::ast::{DeclKind, PatchItem};
use yog_script::parse;

/// The complete examples of the grammar (§7): they must read without a problem.
#[test]
fn the_grammars_examples_read() {
    let doc = include_str!("../../../../docs/design/ys-grammar.md");
    let section = doc
        .split("## 7.")
        .nth(1)
        .and_then(|s| s.split("\n## ").next())
        .unwrap();
    let mut read = 0;
    for (lang, code) in code_blocks(section) {
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

fn code_blocks(text: &str) -> Vec<(String, String)> {
    let mut out = Vec::new();
    let mut parts = text.split("```");
    parts.next();
    while let (Some(block), _) = (parts.next(), parts.next()) {
        let (lang, code) = block.split_once('\n').unwrap();
        out.push((lang.trim().to_string(), code.to_string()));
    }
    out
}

#[test]
fn a_small_mod_reads() {
    let parsed = parse("glassy.ys", include_str!("fixtures/glassy.ys"));
    assert!(parsed.problems.is_empty(), "{:#?}", parsed.problems);
    let kinds: Vec<_> = parsed
        .file
        .decls
        .iter()
        .map(|d| std::mem::discriminant(&d.kind))
        .collect();
    assert_eq!(kinds.len(), 3);
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
    let src = "patch Player {\n    priority 1\n}\n";
    assert_eq!(codes("a.ys", src), [script_problem::PATCH_OUTSIDE_NANITES]);
    assert!(codes("a.yns", src).is_empty());
}

#[test]
fn lexing_problems_are_found_where_they_are() {
    let parsed = parse("a.ys", "let a = \"open\nlet b = 1\n");
    let first = &parsed.problems[0];
    assert_eq!(first.code.as_deref(), Some(script_problem::UNTERMINATED));
    assert_eq!((first.line, first.col), (Some(1), Some(9)));
    assert_eq!(
        codes("a.ys", "let a = @item\n"),
        [script_problem::BAD_REFERENCE]
    );
    assert_eq!(codes("a.ys", "let a = 0x\n"), [script_problem::BAD_NUMBER]);
    assert_eq!(
        codes("a.ys", "let a = 1 $ 2\n"),
        [script_problem::UNEXPECTED_CHAR, script_problem::EXPECTED]
    );
}

/// A broken line is reported and the lines after it still read.
#[test]
fn reading_goes_on_after_a_problem() {
    let src = "fn a() {\n    let = 1\n    let b = 2\n}\nfn c() = 3\n";
    let parsed = parse("a.ys", src);
    assert_eq!(parsed.problems.len(), 1, "{:#?}", parsed.problems);
    assert_eq!(parsed.problems[0].line, Some(2));
    assert_eq!(parsed.file.decls.len(), 2);
}

/// `at call X(x)`: the one name is the argument, not a type list.
#[test]
fn an_argument_point_reads_both_ways() {
    for point in [
        "at call Vec3.add(x) { x * 0.5 }",
        "at call Vec3.add first (x) { x * 0.5 }",
    ] {
        let src = format!(
            "patch LivingEntity {{\n    inside knockback() {{\n        {point}\n    }}\n}}\n"
        );
        let parsed = parse("a.yns", &src);
        assert!(
            parsed.problems.is_empty(),
            "{point}: {:#?}",
            parsed.problems
        );
        let DeclKind::Patch(patch) = &parsed.file.decls[0].kind else {
            panic!()
        };
        assert!(matches!(patch.items[0], PatchItem::Inside { .. }));
    }
}

/// Every word has a description editors can show.
#[test]
fn every_word_is_described() {
    for word in yog_script::words::ALL {
        assert!(!word.doc.is_empty(), "{word:?}");
        assert!(!word.roles.is_empty(), "{word:?}");
    }
}

fn value(src: &str) -> yog_script::ast::Expr {
    let parsed = parse("a.ys", &format!("let v = {src}\n"));
    assert!(parsed.problems.is_empty(), "{src}: {:#?}", parsed.problems);
    let DeclKind::Let { value, .. } = parsed.file.decls.into_iter().next().unwrap().kind else {
        panic!()
    };
    value
}

/// Operators bind by their levels (`ys-grammar.md` §3.2), words and signs alike.
#[test]
fn operators_bind_by_level() {
    use yog_script::ast::{BinOp, ExprKind};
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
    } = value("it is Battery b and b.charge > 0").kind
    else {
        panic!("`and` on top")
    };
    assert!(matches!(left.kind, ExprKind::Is { bind: Some(_), .. }));
    assert!(matches!(
        value("x ?? y ?? z").kind,
        ExprKind::Binary {
            op: BinOp::Coalesce,
            ..
        }
    ));
    assert!(matches!(value("-x as Int").kind, ExprKind::As { .. }));
    assert!(matches!(
        value("x !in 0..10").kind,
        ExprKind::Binary {
            op: BinOp::NotIn,
            ..
        }
    ));
    let ExprKind::Call { args, .. } = value("list.map { it * 2 }").kind else {
        panic!("a trailing lambda")
    };
    assert!(matches!(args[0].value.kind, ExprKind::Lambda { .. }));
    assert!(matches!(value("{ \"a\": 1 }").kind, ExprKind::Map(_)));
    assert!(matches!(
        value("each<Battery> { it }").kind,
        ExprKind::Call { .. }
    ));
    assert!(matches!(
        value("a < b").kind,
        ExprKind::Binary { op: BinOp::Lt, .. }
    ));
}
