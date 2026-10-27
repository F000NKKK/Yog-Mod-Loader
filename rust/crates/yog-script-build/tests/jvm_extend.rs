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

const SAVED: &str = r#"public static class S {
    public static string Run() => "ok";
}
extend Player {
    public saved i32 Visits = 0;
    public saved string Title = "";
    public saved ItemId Favourite = new ItemId("stick");
    public i32 Unsaved = 0;
}
"#;

/// An `extend` keeping `saved` members: each written to its own record,
/// keyed `Player.Member`, and read back into a new instance as it was.
#[test]
fn saved_members_round_trip() {
    let java = r#"((java.util.function.Supplier<String>) () -> {
        try {
            Class<?> ext = java.util.Arrays.stream(t.SScript.class.getDeclaredClasses())
                .filter(c -> c.getSimpleName().startsWith("$ExtendPlayer")).findFirst().get();
            var make = ext.getDeclaredConstructor(String.class);
            make.setAccessible(true);
            Object first = make.newInstance("Steve");
            ext.getField("Visits").setInt(first, 7);
            ext.getField("Title").set(first, "Sir");
            var store = new java.util.TreeMap<String, byte[]>();
            ((dev.yog.script.yog.State.Saved) first).$save(store::put);
            Object again = make.newInstance("Steve");
            ((dev.yog.script.yog.State.Saved) again).$load(store::get);
            return store.keySet() + " " + ext.getField("Visits").getInt(again) + " " + ext.getField("Title").get(again) + " " + ext.getField("Favourite").get(again);
        } catch (ReflectiveOperationException e) {
            throw new RuntimeException(e);
        }
    }).get()"#;
    let Some(out) = support::run(&[("S.ys", SAVED)], java) else {
        return;
    };
    assert_eq!(out, "[Player.Favourite, Player.Title, Player.Visits] 7 Sir minecraft:stick");
}

const MOD: &str = r#"mod Charge {
    public saved i32 Starts = 0;
    public i32 Unsaved = 0;
}
public static class M {
    public static string Run() => "ok";
}
"#;

/// A mod's `saved` members: the mod's class keeps them as `State.Saved`,
/// a record a member (its constructor reads them in from the game's store).
#[test]
fn saved_mod_members() {
    let java = r#"((java.util.function.Supplier<String>) () -> {
        Class<?> mod = java.util.Arrays.stream(t.MScript.class.getDeclaredClasses())
            .filter(c -> c.getSimpleName().equals("Charge")).findFirst().get();
        var records = java.util.Arrays.stream(mod.getDeclaredClasses()).map(Class::getSimpleName).sorted().toList();
        return dev.yog.script.yog.State.Saved.class.isAssignableFrom(mod) + " " + records;
    }).get()"#;
    let Some(out) = support::run(&[("M.ys", MOD)], java) else {
        return;
    };
    assert_eq!(out, "true [$Starts]");
}
