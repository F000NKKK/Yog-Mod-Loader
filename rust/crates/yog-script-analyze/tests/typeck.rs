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
fn the_games_classes_are_a_yns_files_only() {
    let index = game();
    let yns = |src: &str| {
        let src = format!(
            "targets mc 1.21.1..1.21.1;\nusing Game.World.Entity.Player;\nusing Game.Client;\n{src}"
        );
        codes_of(check_with("a.yns", &src, Some(&index)).problems)
    };
    assert!(yns("float F(Player p) => p.Health;\nbool G(Player p) => p.IsAlive;\n").is_empty());
    assert_eq!(
        yns("int F(Player p) => p.Mana;\n"),
        [script_problem::UNKNOWN_MEMBER]
    );
    assert_eq!(
        yns("int F(Player p) => p.Health;\n"),
        [script_problem::TYPE_MISMATCH]
    );
    assert_eq!(
        yns("server void F(Minecraft mc) { }\n"),
        [script_problem::WRONG_SIDE]
    );
    assert!(yns("client void F(Minecraft mc) { }\n").is_empty());
    // What `extend` adds sits beside the game's members.
    assert!(
        yns("extend Player {\n    int Visits = 0;\n}\nint F(Player p) => p.Visits;\n").is_empty()
    );
    // In full, or Yog's where a `using` brings in the game's of the name.
    assert!(yns("float F(Game.World.Entity.Player.Player p) => p.Health;\n").is_empty());
    assert_eq!(
        yns("int F(Yog.Player p) => p.Health;\n"),
        [script_problem::UNKNOWN_MEMBER]
    );
    // A `.ys` reaches Yog, not the game: `Player` is Yog's there.
    let ys = codes_of(check_with("a.ys", "int F(Player p) => p.Mana;\n", Some(&index)).problems);
    assert_eq!(ys, [script_problem::UNKNOWN_MEMBER]);
    // In a `.ys`, `Game` is Yog's `game` group: the game's classes aren't
    // there (types no one declares aren't reported yet).
    let ys = codes_of(
        check_with(
            "a.ys",
            "void F(Game.Client.Minecraft mc) { }\n",
            Some(&index),
        )
        .problems,
    );
    assert!(ys.is_empty(), "{ys:?}");
}

/// A `.ys` reaches Yog: its handles' methods (each action whose first
/// field is the handle's subject), its domains' groups, what they answer.
#[test]
fn yogs_surface_is_typed() {
    let fine = "server void F(Player p, World w) {\n    bool given = p.GiveItem(\"minecraft:diamond\", 1);\n    \
                var info = p.Info();\n    int level = info?.XpLevel ?? 0;\n    bool flies = info!.MayFly;\n    \
                w.SetTime(6000);\n    Chat.Broadcast(\"hello\");\n    var online = Player.Online();\n}\n";
    assert!(codes(fine).is_empty(), "{:?}", codes(fine));
    assert_eq!(
        codes("void F(Player p) {\n    p.GiveItem(1);\n}\n"),
        [script_problem::WRONG_ARGS]
    );
    assert_eq!(
        codes("void F(Player p) {\n    p.Fly();\n}\n"),
        [script_problem::UNKNOWN_MEMBER]
    );
    assert_eq!(
        codes("void F(Player p) {\n    int k = p.Kick(\"bye\");\n}\n"),
        [script_problem::TYPE_MISMATCH]
    );
    assert_eq!(
        codes("int F(Player p) => p.Info()!.Mana;\n"),
        [script_problem::UNKNOWN_MEMBER]
    );
    assert_eq!(
        codes("void F() {\n    Chat.Shout(\"hi\");\n}\n"),
        [script_problem::UNKNOWN_MEMBER]
    );
}
