//! `extend` on the JVM: members added to the game's objects (a player, the
//! world), each owner's state its own.

mod support;

const SRC: &str = r#"public static class E {
    public static string Run() {
        let steve = new Player("Steve");
        let alex = new Player("Alex");
        steve.Visit();
        steve.Visit();
        alex.Visit();
        let w = new World("minecraft:overworld");
        w.Ticks += 5;
        new World("minecraft:the_nether").Ticks += 1;
        return steve.Visits + " " + alex.Visits + " " + steve.Greeting + " " + w.Ticks;
    }
}
extend Player {
    public i32 Visits = 0;
    public string Greeting => "visited " + Visits;
    public void Visit() { Visits += 1; }
}
extend World {
    public i32 Ticks = 0;
}
"#;

/// Each player's own `Visits`; the world's one `Ticks` (every dimension's).
#[test]
fn extensions_hold_each_owners_state() {
    let Some(out) = support::run(&[("E.ys", SRC)], "t.EScript.E.Run()") else {
        return;
    };
    assert_eq!(out, "2 1 visited 2 6");
}
