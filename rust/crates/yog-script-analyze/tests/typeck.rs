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
        codes("void F() {\n    i32 a = \"x\";\n}\n"),
        [script_problem::TYPE_MISMATCH]
    );
    assert_eq!(codes("i32 F() => true;\n"), [script_problem::TYPE_MISMATCH]);
    assert_eq!(
        codes("void F(bool b) {\n    if (1 + 2) b = false;\n}\n"),
        [script_problem::TYPE_MISMATCH]
    );
    // Widening, literals of any number, units.
    let fine =
        "f64 F(i32 a) {\n    i64 b = a;\n    f32 c = 2.0;\n    Ticks t = 5s;\n    return a;\n}\n";
    assert!(codes(fine).is_empty(), "{:?}", codes(fine));
    assert_eq!(
        codes("void F() {\n    Seconds s = 5t;\n}\n"),
        [script_problem::TYPE_MISMATCH]
    );
}

#[test]
fn null_needs_a_nullable() {
    assert_eq!(
        codes("record Pos(i32 X);\nvoid F() {\n    Pos p = null;\n}\n"),
        [script_problem::NULLABLE]
    );
    assert!(codes("record Pos(i32 X);\nvoid F() {\n    Pos? p = null;\n}\n").is_empty());
    let src = "record Pos(i32 X);\ni32 F(Pos? p) => p.X;\n";
    assert_eq!(codes(src), [script_problem::NULLABLE]);
    assert!(codes("record Pos(i32 X);\ni32 F(Pos? p) => p?.X ?? 0;\n").is_empty());
}

#[test]
fn members_and_calls_are_the_types() {
    let src = "record Pos(i32 X) {\n    i32 Twice() => X * 2;\n}\ni32 F(Pos p) => p.Y;\n";
    assert_eq!(codes(src), [script_problem::UNKNOWN_MEMBER]);
    let src = "i32 Area(i32 w, i32 h) => w * h;\ni32 F() => Area(1);\n";
    assert_eq!(codes(src), [script_problem::WRONG_ARGS]);
    let src = "i32 Area(i32 w, i32 h) => w * h;\ni32 F() => Area(1, \"2\");\n";
    assert_eq!(codes(src), [script_problem::TYPE_MISMATCH]);
    // A component's members on its owner.
    let src = "component Charge(i32 capacity) {\n    public i32 Energy = 0;\n}\n\
               block Charger has Charge(10) { }\ni32 F(Charger c) => c.Energy + c.Charge.Energy;\n";
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
            "targets mc 1.21.1..1.21.1;\nusing Player = System.Game.World.Entity.Player.Player;\nusing System.Game.Client;\n{src}"
        );
        codes_of(check_with("a.yns", &src, Some(&index)).problems)
    };
    assert!(yns("f32 F(Player p) => p.Health;\nbool G(Player p) => p.IsAlive;\n").is_empty());
    assert_eq!(
        yns("i32 F(Player p) => p.Mana;\n"),
        [script_problem::UNKNOWN_MEMBER]
    );
    assert_eq!(
        yns("i32 F(Player p) => p.Health;\n"),
        [script_problem::TYPE_MISMATCH]
    );
    assert_eq!(
        yns("server void F(Minecraft mc) { }\n"),
        [script_problem::WRONG_SIDE]
    );
    assert!(yns("client void F(Minecraft mc) { }\n").is_empty());
    // What `extend` adds sits beside the game's members.
    assert!(
        yns("extend Player {\n    public i32 Visits = 0;\n}\ni32 F(Player p) => p.Visits;\n")
            .is_empty()
    );
    // As in C#, a name two `using`s bring in (`System.Yog`, every file's, among
    // them) is ambiguous: an alias, as above, or the full name settles it.
    let ambiguous = "targets mc 1.21.1..1.21.1;\nusing System.Game.World.Entity.Player;\nf32 F(Player p) => 1;\n";
    let found = codes_of(check_with("a.yns", ambiguous, Some(&index)).problems);
    assert_eq!(found, [script_problem::AMBIGUOUS]);
    assert!(yns("f32 F(System.Game.World.Entity.Player.Player p) => p.Health;\n").is_empty());
    assert_eq!(
        yns("i32 F(System.Yog.Player p) => p.Health;\n"),
        [script_problem::UNKNOWN_MEMBER]
    );
    // A `.ys` reaches Yog, not the game: `Player` is Yog's there.
    let ys = codes_of(check_with("a.ys", "i32 F(Player p) => p.Mana;\n", Some(&index)).problems);
    assert_eq!(ys, [script_problem::UNKNOWN_MEMBER]);
    // A `.ys` naming `System.Game` is told the game's classes are a `.yns`'s.
    let ys = codes_of(
        check_with(
            "a.ys",
            "void F(System.Game.Client.Minecraft mc) { }\n",
            Some(&index),
        )
        .problems,
    );
    assert_eq!(ys, [script_problem::TARGETS]);
}

/// A `.ys` reaches Yog: its handles' methods (each action whose first
/// field is the handle's subject), its domains' groups, what they answer.
#[test]
fn yogs_surface_is_typed() {
    let fine = "server void F(Player p, World w) {\n    bool given = p.GiveItem(\"minecraft:diamond\", 1);\n    \
                let info = p.Info();\n    i32 level = info?.XpLevel ?? 0;\n    bool flies = info!.MayFly;\n    \
                w.SetTime(6000);\n    Chat.Broadcast(\"hello\");\n    let online = Player.Online();\n}\n";
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
        codes("void F(Player p) {\n    i32 k = p.Kick(\"bye\");\n}\n"),
        [script_problem::TYPE_MISMATCH]
    );
    assert_eq!(
        codes("i32 F(Player p) => p.Info()!.Mana;\n"),
        [script_problem::UNKNOWN_MEMBER]
    );
    assert_eq!(
        codes("void F() {\n    Chat.Shout(\"hi\");\n}\n"),
        [script_problem::UNKNOWN_MEMBER]
    );
}

/// `namespace` and `using` as in C#.
#[test]
fn namespaces_and_usings_work_as_in_csharp() {
    let src = "namespace Charge.Machines;\n\nrecord Pos(i32 X);\ni32 F(Pos p) => p.X;\ni32 G(Charge.Machines.Pos p) => p.Y;\n";
    assert_eq!(codes(src), [script_problem::UNKNOWN_MEMBER]);
    let src = "namespace Charge {\n    record Pos(i32 X);\n}\nnamespace Other {\n    using Charge;\n    i32 F(Pos p) => p.Y;\n}\n";
    assert_eq!(codes(src), [script_problem::UNKNOWN_MEMBER]);
    // An alias of a namespace; the static members of a type.
    let src = "using Y = System.Yog;\nusing static System.Yog.Chat;\nvoid F(Y.Player p) {\n    bool sent = Broadcast(\"hi\");\n    Broadcast(1, 2);\n}\n";
    assert_eq!(codes(src), [script_problem::WRONG_ARGS]);
}

/// Events as C#'s: declared, subscribed (`+=` a lambda typed by the event,
/// or a method of its signature), raised; Yog's are its groups'.
#[test]
fn events_work_as_in_csharp() {
    let fine = "mod M {\n    M() {\n        Player.Join += e => Chat.Broadcast(e.PlayerName);\n        \
                Block.Break += OnBreak;\n    }\n    void OnBreak(Block.BreakArgs e) { }\n}\n";
    assert!(codes(fine).is_empty(), "{:?}", codes(fine));
    // The lambda's parameter is the event's arguments: their members only.
    assert_eq!(
        codes("mod M {\n    M() {\n        Player.Join += e => e.Mana;\n    }\n}\n"),
        [script_problem::UNKNOWN_MEMBER]
    );
    // A method of another signature isn't one.
    let src = "mod M {\n    M() {\n        Player.Join += OnBreak;\n    }\n    void OnBreak(Block.BreakArgs e) { }\n}\n";
    assert_eq!(codes(src), [script_problem::TYPE_MISMATCH]);
    // An event of one's own: declared, subscribed, raised.
    let own = "class Battery {\n    public event Action<i32> Charged;\n    void Fill() {\n        Charged += n => { i32 twice = n * 2; };\n        \
               Charged?.Invoke(3);\n        Charged(\"three\");\n    }\n}\n";
    assert_eq!(codes(own), [script_problem::TYPE_MISMATCH]);
}

#[test]
fn classes_inherit_as_in_csharp() {
    let types = "interface ITick { void Tick(); }\nclass Machine : ITick { public i32 Rate = 1; public void Tick() {} }\n\
                 class Battery : Machine { public i32 Boost() => base.Rate + Rate; }\nclass Other {}\n";
    // Members come from the base class; a derived object goes where its base is wanted.
    let ok = format!(
        "{types}void F() {{ let mut b = new Battery(); i32 r = b.Rate; b.Tick(); Machine m = move b; ITick t = new Battery(); }}\n"
    );
    assert!(codes(&ok).is_empty(), "{:?}", codes(&ok));
    // Not the other way, nor an unrelated type.
    let bad =
        format!("{types}void F() {{ Battery b = new Machine(); Machine m = new Other(); }}\n");
    assert_eq!(
        codes(&bad),
        [script_problem::TYPE_MISMATCH, script_problem::TYPE_MISMATCH]
    );
}

#[test]
fn inheritance_follows_csharps_rules() {
    let base = "interface ITick { void Tick(); }\nabstract class Machine { public abstract i32 Rate(); \
                public virtual void Glow() {} }\nsealed class Final {}\n";
    let ok = format!(
        "{base}class Battery : Machine, ITick {{ public override i32 Rate() => 4; public override void Glow() {{}} \
         public void Tick() {{}} }}\n"
    );
    assert!(codes(&ok).is_empty(), "{:?}", codes(&ok));
    let cases = [
        // An abstract member left, an interface member left.
        "class A : Machine, ITick {}",
        // The base class after an interface.
        "class A : ITick, Machine { public override i32 Rate() => 1; public void Tick() {} }",
        // A sealed class inherited.
        "class A : Final {}",
        // An override of nothing.
        "class A { public override void Glow() {} }",
        // A struct inheriting a class.
        "struct A : Final {}",
    ];
    let counts = [2, 1, 1, 1, 1];
    for (case, count) in cases.iter().zip(counts) {
        let found = codes(&format!("{base}{case}\n"));
        assert_eq!(found, vec![script_problem::INHERIT; count], "{case}");
    }
    // An abstract class isn't made.
    assert_eq!(
        codes(&format!("{base}void F() {{ let m = new Machine(); }}\n")),
        [script_problem::INHERIT]
    );
}

#[test]
fn enums_are_rusts() {
    let mode = "enum Mode { Idle, Charging(i32 Rate), Full }\n";
    let ok = format!(
        "{mode}string F(Mode m) => m switch {{ Mode.Idle => \"idle\", Mode.Charging(let r) when r > 3 => \"fast\", \
         Mode.Charging(let r) => \"slow\", Mode.Full => \"full\" }};\nMode G() => Mode.Charging(4);\n"
    );
    assert!(codes(&ok).is_empty(), "{:?}", codes(&ok));
    // A case's data has its type; a case left out is an error.
    assert_eq!(
        codes(&format!("{mode}Mode G() => Mode.Charging(\"x\");\n")),
        [script_problem::TYPE_MISMATCH]
    );
    assert_eq!(
        codes(&format!(
            "{mode}string F(Mode m) => m switch {{ Mode.Idle => \"a\", Mode.Charging(let r) when r > 3 => \"b\", Mode.Full => \"c\" }};\n"
        )),
        [script_problem::NOT_EXHAUSTIVE]
    );
    assert!(
        codes(&format!(
            "{mode}string F(Mode m) => m switch {{ Mode.Idle => \"a\", _ => \"b\" }};\n"
        ))
        .is_empty()
    );
    // What a case's pattern binds is typed.
    assert_eq!(
        codes(&format!(
            "{mode}string F(Mode m) => m switch {{ Mode.Charging(let r) => r, _ => \"b\" }};\n"
        )),
        [script_problem::TYPE_MISMATCH]
    );
}

#[test]
fn generics_are_csharps() {
    let types = "class Box<T> { public T Value; public void Put(T v) {} public T Get() => Value; }\n\
                 static T Max<T>(T a, T b) => a;\n";
    // A generic's members with its arguments; a method's inferred from the call.
    let ok = format!(
        "{types}void F(Box<i32> b) {{ i32 v = b.Value; i32 g = b.Get(); b.Put(3); i32 m = Max(1, 2); string s = Max(\"a\", \"b\"); }}\n"
    );
    assert!(codes(&ok).is_empty(), "{:?}", codes(&ok));
    let bad = format!(
        "{types}void F(Box<i32> b) {{ string v = b.Value; b.Put(\"x\"); string m = Max(1, 2); }}\n"
    );
    assert_eq!(
        codes(&bad),
        [
            script_problem::TYPE_MISMATCH,
            script_problem::TYPE_MISMATCH,
            script_problem::TYPE_MISMATCH
        ]
    );
}

#[test]
fn values_copy_and_objects_move() {
    let types = "record struct Pos(i32 X, i32 Y);\nstruct Bag(List<i32> items) { public List<i32> Items = items; }\n\
                 class Box {}\nstatic void Keep(Box b) {}\n";
    // A `Copy` struct copies; an object, a struct holding a collection, a list move.
    assert!(
        codes(&format!(
            "{types}void F(Pos p) {{ let q = p; Pos r = p; }}\n"
        ))
        .is_empty()
    );
    let bad = format!(
        "{types}void F(Box b, Bag g, List<i32> xs) {{ let c = b; let h = g; let ys = xs; Keep(b); }}\n"
    );
    assert_eq!(codes(&bad), vec![script_problem::PASS; 4]);
    let ok = format!("{types}void F(Box b, Box c) {{ let d = move b; Keep(move c); }}\n");
    assert!(codes(&ok).is_empty(), "{:?}", codes(&ok));
}

#[test]
fn what_is_lent_read_only_isnt_changed() {
    let types = "class Box { public i32 N; public readonly i32 Get() => N; public void Bump() { N += 1; } \
                 public readonly void Peek() { Bump(); } }\nreadonly struct P(i32 x) { public i32 X = x; public void M() { X = 1; } }\n";
    let bad = format!(
        "{types}void F(in Box i) {{ let b = new Box(); b.N = 1; b.Bump(); i.Bump(); i.N = 2; }}\n"
    );
    // A readonly member's `this` (Peek, P.M) and each write through a let or an in.
    assert_eq!(
        codes(&bad),
        vec![script_problem::BORROW; 6],
        "{:?}",
        codes(&bad)
    );
    let ok = format!(
        "{types}void F(in Box i, ref Box r) {{ let mut b = new Box(); b.N = 1; b.Bump(); i32 n = i.Get(); r.Bump(); }}\n"
    );
    assert_eq!(
        codes(&ok),
        vec![script_problem::BORROW; 2],
        "{:?}",
        codes(&ok)
    );
}

#[test]
fn records_never_change() {
    let types = "record Pos(i32 X, i32 Y);\n";
    // Shared freely (Copy), never written, even through a `let mut`.
    let src = format!("{types}void F(Pos p) {{ let q = p; let mut r = p; r.X = 2; }}\n");
    assert_eq!(codes(&src), [script_problem::BORROW], "{:?}", codes(&src));
    // Not by its own members either.
    let own = "record Pos(i32 X) { public void Move() { X = 1; } }\n";
    assert_eq!(codes(own), [script_problem::ASSIGN_TO_CONSTANT]);
}

#[test]
fn sending_to_the_server_is_declared() {
    let src = "void F(List<u8> bytes) { bool sent = Net.SendToServer(\"m:c\", move bytes); }\n";
    let undeclared = yog_script_analyze::check_in("a.ys", src, yog_script_analyze::Env::default());
    assert_eq!(codes_of(undeclared.problems), [script_problem::UNDECLARED]);
    let env = yog_script_analyze::Env {
        to_server: true,
        ..Default::default()
    };
    let declared = yog_script_analyze::check_in("a.ys", src, env);
    assert!(codes_of(declared.problems).is_empty());
}

#[test]
fn an_indexer_is_called_by_brackets() {
    let grid = "class Grid {\n    public i32 this[i32 i] { get => i * 2; }\n}\n";
    let fine = format!("{grid}i32 F(Grid g) => g[3];\n");
    assert!(codes(&fine).is_empty(), "{:?}", codes(&fine));
    assert_eq!(
        codes(&format!("{grid}i32 F(Grid g) => g[\"a\"];\n")),
        [script_problem::TYPE_MISMATCH]
    );
    assert_eq!(
        codes(&format!("{grid}string F(Grid g) => g[1];\n")),
        [script_problem::TYPE_MISMATCH]
    );
}

#[test]
fn a_get_only_indexer_isnt_written() {
    let grid = "class Grid {\n    public i32 this[i32 i] { get => i; }\n}\n";
    assert_eq!(
        codes(&format!("{grid}void F(Grid g) {{\n    g[0] = 1;\n}}\n")),
        [script_problem::ASSIGN_TO_CONSTANT]
    );
    let set = "class Grid {\n    public i32 this[i32 i] { get => i; set { } }\n}\n";
    let fine = format!("{set}void F(Grid g) {{\n    g[0] = 1;\n}}\n");
    assert!(codes(&fine).is_empty(), "{:?}", codes(&fine));
}

#[test]
fn a_setter_takes_its_value() {
    let src = "class Grid {\n    List<i32> _cells = [0, 0];\n    public i32 this[i32 i] {\n        get => _cells[i];\n        set { _cells[i] = value; }\n    }\n}\n";
    assert!(codes(src).is_empty(), "{:?}", codes(src));
}

#[test]
fn inherited_members_take_their_bases_arguments() {
    let types = "interface IBox<T> {\n    T Get();\n}\n\
                 class IntBox : IBox<i32> {\n    public i32 Get() => 1;\n}\n\
                 interface IPair<TFirst, TSecond> : IBox<TSecond> {\n    TFirst First();\n}\n";
    let fine =
        format!("{types}i32 F(IntBox b) => b.Get();\nstring G(IPair<i32, string> p) => p.Get();\n");
    assert!(codes(&fine).is_empty(), "{:?}", codes(&fine));
    assert_eq!(
        codes(&format!(
            "{types}string F(IPair<i32, string> p) => p.First();\n"
        )),
        [script_problem::TYPE_MISMATCH]
    );
}

#[test]
fn a_type_is_its_name_and_arity() {
    let types =
        "class Box {\n    public i32 Size = 0;\n}\nclass Box<T> {\n    public T Value;\n}\n";
    let fine = format!("{types}i32 F(Box b) => b.Size;\nstring G(Box<string> b) => b.Value;\n");
    assert!(codes(&fine).is_empty(), "{:?}", codes(&fine));
    assert_eq!(
        codes(&format!("{types}i32 F(Box<string> b) => b.Size;\n")),
        [script_problem::UNKNOWN_MEMBER]
    );
}

#[test]
fn a_delegate_is_a_type_of_methods() {
    let fine = "delegate i32 Scale(i32 x);\n\
                bool Even(i32 n) => n % 2 == 0;\n\
                i32 F() {\n    Scale twice = x => x * 2;\n    Predicate<i32> even = Even;\n    return twice(3) + twice.Invoke(4);\n}\n\
                bool G(Func<i32, string, bool> f) => f(1, \"a\");\n";
    assert!(codes(fine).is_empty(), "{:?}", codes(fine));
    assert_eq!(
        codes("delegate i32 Scale(i32 x);\nstring F(Scale s) => s(1);\n"),
        [script_problem::TYPE_MISMATCH]
    );
    assert_eq!(
        codes("delegate i32 Scale(i32 x);\nvoid F() {\n    Scale s = (string x) => 1;\n}\n"),
        [script_problem::TYPE_MISMATCH]
    );
}

#[test]
fn an_event_is_of_any_delegate_type() {
    let src = "class Args : EventArgs {\n    public i32 Count = 0;\n}\n\
               class Button {\n    public event EventHandler<Args> Clicked;\n    \
               public void Click() { Clicked?.Invoke(this, new Args()); }\n}\n\
               void F(Button b) {\n    b.Clicked += (sender, e) => { i32 n = e.Count; };\n}\n";
    assert!(codes(src).is_empty(), "{:?}", codes(src));
}

const ENERGY: &str = "public struct Energy(i32 amount) {\n    public i32 Amount = amount;\n\
    public static Energy operator +(Energy a, Energy b) => new Energy(a.Amount + b.Amount);\n\
    public static Energy operator -(Energy a) => new Energy(-a.Amount);\n\
    public static bool operator ==(Energy a, Energy b) => a.Amount == b.Amount;\n\
    public static bool operator !=(Energy a, Energy b) => a.Amount != b.Amount;\n\
    public static implicit operator Energy(i32 amount) => new Energy(amount);\n\
    public static explicit operator i32(Energy e) => e.Amount;\n}\n";

#[test]
fn a_types_operators_are_its_own() {
    let fine = format!(
        "{ENERGY}Energy Sum(Energy a, Energy b) => a + b;\nEnergy Neg(Energy a) => -a;\n\
         bool Same(Energy a, Energy b) => a == b;\nEnergy Five() => 5;\ni32 Back(Energy e) => (i32)e;\n\
         Energy Twice(Energy e) => Sum(e, 3);\n"
    );
    assert!(codes(&fine).is_empty(), "{:?}", codes(&fine));
    // No `operator *`: two Energies don't multiply.
    assert_eq!(
        codes(&format!("{ENERGY}Energy F(Energy a, Energy b) => a * b;\n")),
        [script_problem::TYPE_MISMATCH]
    );
    // An explicit conversion isn't implicit.
    assert_eq!(
        codes(&format!("{ENERGY}i32 F(Energy e) => e;\n")),
        [script_problem::TYPE_MISMATCH]
    );
}

#[test]
fn operators_are_declared_as_csharps() {
    let src = "public struct V(i32 x) {\n    public i32 X = x;\n\
               public static bool operator ==(V a, V b) => a.X == b.X;\n\
               public V operator -(V a) => a;\n\
               public static i32 operator +(i32 a, i32 b) => a;\n}\n";
    assert_eq!(
        codes(src),
        [
            script_problem::OPERATOR,
            script_problem::OPERATOR,
            script_problem::OPERATOR
        ]
    );
}

#[test]
fn methods_overload_by_their_parameters() {
    let ok = "class Scale {\n    public f64 Of(f64 x) => x;\n    public f64 Of(in Scale s) => 1.0;\n}\n\
              f64 F(Scale s) => s.Of(2.0) + s.Of(in s);\n";
    assert!(codes(ok).is_empty(), "{:?}", codes(ok));
    // The same parameters twice, as C#'s CS0111; a field isn't a method.
    for bad in [
        "class Scale {\n    public f64 Of(f64 x) => x;\n    public f64 Of(f64 y) => y;\n}\n",
        "class Scale {\n    public i32 N;\n    public void N() {}\n}\n",
    ] {
        assert_eq!(codes(bad), [script_problem::DUPLICATE], "{bad}");
    }
}
