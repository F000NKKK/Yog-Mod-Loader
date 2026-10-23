//! Types checked (`yog_script_analyze::check_with`): what doesn't fit.

use yog_ids::script_problem;
use yog_script_analyze::index::{Class, ClassIndex, Member, Side, access};
use yog_script_analyze::{check, check_with};

fn codes(src: &str) -> Vec<String> {
    codes_of(check("a.ys", src).problems)
}

fn codes_of(problems: Vec<yog_data::Diagnostic>) -> Vec<String> {
    problems
        .into_iter()
        .filter_map(|d| d.code.map(|c| c.to_string()))
        .collect()
}

#[test]
fn values_fit_their_types() {
    assert_eq!(
        codes("void F() {\n    int a = \"x\";\n}\n"),
        [script_problem::TYPE_MISMATCH]
    );
    assert_eq!(codes("int F() => true;\n"), [script_problem::TYPE_MISMATCH]);
    assert_eq!(
        codes("void F(bool b) {\n    if (1 + 2) b = false;\n}\n"),
        [script_problem::TYPE_MISMATCH]
    );
    // Widening, literals of any number, units.
    let fine = "double F(int a) {\n    long b = a;\n    float c = 2.0;\n    Ticks t = 5s;\n    return a;\n}\n";
    assert!(codes(fine).is_empty(), "{:?}", codes(fine));
    assert_eq!(
        codes("void F() {\n    Seconds s = 5t;\n}\n"),
        [script_problem::TYPE_MISMATCH]
    );
}

#[test]
fn null_needs_a_nullable() {
    assert_eq!(
        codes("record Pos(int X);\nvoid F() {\n    Pos p = null;\n}\n"),
        [script_problem::NULLABLE]
    );
    assert!(codes("record Pos(int X);\nvoid F() {\n    Pos? p = null;\n}\n").is_empty());
    let src = "record Pos(int X);\nint F(Pos? p) => p.X;\n";
    assert_eq!(codes(src), [script_problem::NULLABLE]);
    assert!(codes("record Pos(int X);\nint F(Pos? p) => p?.X ?? 0;\n").is_empty());
}

#[test]
fn members_and_calls_are_the_types() {
    let src = "record Pos(int X) {\n    int Twice() => X * 2;\n}\nint F(Pos p) => p.Y;\n";
    assert_eq!(codes(src), [script_problem::UNKNOWN_MEMBER]);
    let src = "int Area(int w, int h) => w * h;\nint F() => Area(1);\n";
    assert_eq!(codes(src), [script_problem::WRONG_ARGS]);
    let src = "int Area(int w, int h) => w * h;\nint F() => Area(1, \"2\");\n";
    assert_eq!(codes(src), [script_problem::TYPE_MISMATCH]);
    // A component's members on its owner.
    let src = "component Charge(int capacity) {\n    int Energy = 0;\n}\n\
               block Charger has Charge(10) { }\nint F(Charger c) => c.Energy + c.Charge.Energy;\n";
    assert!(codes(src).is_empty(), "{:?}", codes(src));
}

/// The game's classes by C#'s names, and their sides.
fn game() -> ClassIndex {
    let m = |name: &str, desc: &str| Member {
        name: name.into(),
        desc: desc.into(),
        access: access::PUBLIC,
    };
    let class = |name: &str, side, methods| Class {
        name: name.into(),
        side,
        access: access::PUBLIC,
        extends: None,
        implements: vec![],
        fields: vec![],
        methods,
    };
    ClassIndex {
        mc: "1.21.1".into(),
        classes: vec![
            class(
                "net/minecraft/world/entity/player/Player",
                Side::Shared,
                vec![m("getHealth", "()F"), m("isAlive", "()Z")],
            ),
            class("net/minecraft/client/Minecraft", Side::Client, vec![]),
        ],
    }
}

#[test]
fn the_games_classes_are_checked_by_their_index() {
    let index = game();
    let check = |src: &str| codes_of(check_with("a.ys", src, Some(&index)).problems);
    assert!(check("float F(Player p) => p.Health;\nbool G(Player p) => p.IsAlive;\n").is_empty());
    assert_eq!(
        check("int F(Player p) => p.Mana;\n"),
        [script_problem::UNKNOWN_MEMBER]
    );
    assert_eq!(
        check("int F(Player p) => p.Health;\n"),
        [script_problem::TYPE_MISMATCH]
    );
    assert_eq!(
        check("server void F(Minecraft mc) { }\n"),
        [script_problem::WRONG_SIDE]
    );
    assert!(check("client void F(Minecraft mc) { }\n").is_empty());
    // What `extend` adds sits beside the game's members.
    assert!(
        check("extend Player {\n    int Visits = 0;\n}\nint F(Player p) => p.Visits;\n").is_empty()
    );
}
