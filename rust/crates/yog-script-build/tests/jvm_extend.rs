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

const SAVED: &str = r#"using System.Collections.Generic;
public enum Mode { Idle, Busy }
public record Spot(i32 X, i32 Y, string Name);
public enum Shape { Dot, Line(i32 Len, string Tag) }
public static class S {
    public static string Run() => "ok";
}
extend Player {
    public saved i32 Visits = 0;
    public saved string Title = "";
    public saved ItemId Favourite = new ItemId("stick");
    public saved string? Note = null;
    public saved List<i32> Scores = new List<i32>();
    public saved Mode Mode = Mode.Idle;
    public saved Spot Home = new Spot(0, 0, "");
    public saved Dictionary<Spot, i32> Marks = new Dictionary<Spot, i32>();
    public saved Shape Drawn = Shape.Dot;
    public saved Shape Kept = Shape.Dot;
    public saved Text Greeting = Text.Of("m.hello").With(3, "Steve", Text.Of("m.inner"));
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
            ext.getField("Note").set(first, "hi");
            ((java.util.List<Integer>) ext.getField("Scores").get(first)).add(5);
            ((java.util.List<Integer>) ext.getField("Scores").get(first)).add(9);
            ext.getField("Mode").set(first, t.SScript.Mode.Busy);
            ext.getField("Home").set(first, new t.SScript.Spot(3, 4, "base"));
            ((dev.yog.script.collections.generic.Dictionary<Object, Integer>) ext.getField("Marks").get(first)).set_Item(new t.SScript.Spot(1, 2, "x"), 8);
            ext.getField("Drawn").set(first, new t.SScript.Shape.Line(4, "edge"));
            var store = new java.util.TreeMap<String, byte[]>();
            ((dev.yog.script.yog.State.Saved) first).$save(store::put);
            Object again = make.newInstance("Steve");
            ((dev.yog.script.yog.State.Saved) again).$load(store::get);
            return store.keySet() + " " + ext.getField("Visits").getInt(again) + " " + ext.getField("Title").get(again) + " " + ext.getField("Favourite").get(again)
                + " " + ext.getField("Note").get(again) + " " + ext.getField("Scores").get(again).getClass().getSimpleName() + ext.getField("Scores").get(again) + " " + ext.getField("Mode").get(again) + " " + ext.getField("Home").get(again) + " " + ext.getField("Marks").get(again) + " " + ext.getField("Drawn").get(again) + " " + ext.getField("Kept").get(again)
                + " " + ((dev.yog.script.yog.Text) ext.getField("Greeting").get(again)).json();
        } catch (ReflectiveOperationException e) {
            throw new RuntimeException(e);
        }
    }).get()"#;
    let Some(out) = support::run(&[("S.ys", SAVED)], java) else {
        return;
    };
    assert_eq!(
        out,
        "[Player.Drawn, Player.Favourite, Player.Greeting, Player.Home, Player.Kept, Player.Marks, Player.Mode, Player.Note, Player.Scores, Player.Title, Player.Visits] 7 Sir minecraft:stick hi List[5, 9] Busy Spot { X = 3, Y = 4, Name = base } {Spot { X = 1, Y = 2, Name = x }=8} Line(4, edge) Dot {\"translate\":\"m.hello\",\"with\":[\"3\",\"Steve\",{\"translate\":\"m.inner\"}]}"
    );
}

const WAS: &str = r#"public static class W {
    public static string Run() => "ok";
}
extend Player {
    public saved i32 Visits = 0 was Count;
    public saved i64 Spent = 0 was Paid: i32;
}
"#;

/// A member's `was`: with nothing under its key, it's read from what an
/// old save called it (as its type then, cast to the type now), and the
/// old record is dropped.
#[test]
fn was_reads_an_old_saves_records() {
    let java = r#"((java.util.function.Supplier<String>) () -> {
        try {
            Class<?> ext = java.util.Arrays.stream(t.WScript.class.getDeclaredClasses())
                .filter(c -> c.getSimpleName().startsWith("$ExtendPlayer")).findFirst().get();
            Class<?> count = java.util.Arrays.stream(ext.getDeclaredClasses()).filter(c -> c.getSimpleName().equals("$Visits")).findFirst().get();
            Class<?> paid = java.util.Arrays.stream(ext.getDeclaredClasses()).filter(c -> c.getSimpleName().equals("$Spent$was")).findFirst().get();
            var store = new java.util.TreeMap<String, byte[]>();
            store.put("Player.Count", dev.yog.api.YogBus.bytes(count.getDeclaredConstructor(int.class).newInstance(7)));
            store.put("Player.Paid", dev.yog.api.YogBus.bytes(paid.getDeclaredConstructor(int.class).newInstance(12)));
            var make = ext.getDeclaredConstructor(String.class);
            make.setAccessible(true);
            Object again = make.newInstance("Steve");
            ((dev.yog.script.yog.State.Saved) again).$load(new dev.yog.script.yog.State.Records() {
                public byte[] read(String key) { return store.get(key); }
                public void drop(String key) { store.remove(key); }
            });
            return store.keySet() + " " + ext.getField("Visits").getInt(again) + " " + ext.getField("Spent").getLong(again);
        } catch (ReflectiveOperationException e) {
            throw new RuntimeException(e);
        }
    }).get()"#;
    let Some(out) = support::run(&[("W.ys", WAS)], java) else {
        return;
    };
    assert_eq!(out, "[] 7 12");
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

const SYNCED: &str = r#"using System.Collections.Generic;
public static class Y {
    public static string Run() => "ok";
}
extend Player {
    public synced i32 Charge = 0;
    public saved synced string Rank = "";
    public synced List<string> Tags = new List<string>();
}
mod Charger {
    public synced i32 Online = 0;
}
"#;

/// `synced` members: an object writes each out as a row (`$sync`), and
/// another of the same owner (a client's) takes them in (`$synced`).
#[test]
fn synced_members_reach_another_object() {
    let java = r#"((java.util.function.Supplier<String>) () -> {
        try {
            Class<?> ext = java.util.Arrays.stream(t.YScript.class.getDeclaredClasses())
                .filter(c -> c.getSimpleName().startsWith("$ExtendPlayer")).findFirst().get();
            var make = ext.getDeclaredConstructor(String.class);
            make.setAccessible(true);
            Object server = make.newInstance("Steve");
            ext.getField("Charge").setInt(server, 40);
            ext.getField("Rank").set(server, "gold");
            ((java.util.List<String>) ext.getField("Tags").get(server)).add("fast");
            var rows = new java.util.TreeMap<String, byte[]>();
            ((dev.yog.script.yog.SyncedState.Synced) server).$sync(rows::put);
            Object client = make.newInstance("Steve");
            rows.forEach(((dev.yog.script.yog.SyncedState.Synced) client)::$synced);
            boolean mod = java.util.Arrays.stream(t.YScript.class.getDeclaredClasses())
                .anyMatch(c -> c.getSimpleName().equals("Charger") && dev.yog.script.yog.SyncedState.Synced.class.isAssignableFrom(c));
            return rows.keySet() + " " + ext.getField("Charge").getInt(client) + " " + ext.getField("Rank").get(client) + " " + ext.getField("Tags").get(client) + " " + mod;
        } catch (ReflectiveOperationException e) {
            throw new RuntimeException(e);
        }
    }).get()"#;
    let Some(out) = support::run(&[("Y.ys", SYNCED)], java) else {
        return;
    };
    assert_eq!(
        out,
        "[Player.Charge, Player.Rank, Player.Tags] 40 gold [fast] true"
    );
}

const RELOADED: &str = r#"using System.Collections.Generic;
using System.Threading.Tasks;
public class Thing(i32 n) {
    public i32 N = n;
}
public static class K {
    public static async Task<string> Wait(Task gate) {
        let p = new Player("Steve");
        await gate;
        p.Combo += 1;
        return "done " + p.Combo + " " + p.Last + " " + p.Fixed + " " + p.Bag.Count + " " + p.Bag[0].N;
    }
}
extend Player {
    public i32 Combo = 0;
    public string Last = "";
    public readonly i32 Fixed = 1;
    public List<Thing> Bag = new List<Thing>();
}
"#;

/// A hot reload carries everything over (storage.md S8, S9): the same
/// classes loaded again as a new generation, an owner's instance's every
/// member — plain, readonly, a list of the mod's own class — is the new
/// instance's, and an async method waiting mid-way goes on in its
/// machine's new class, reaching the new instance.
#[test]
fn a_hot_reload_carries_state_and_waiting_methods() {
    let java = r#"((java.util.function.Supplier<String>) () -> {
        try {
            ClassLoader app = t.KScript.class.getClassLoader();
            java.net.URL dir = t.KScript.class.getProtectionDomain().getCodeSource().getLocation();
            // A generation of the mod's classes: t.* loaded apart, the runtime shared (as in the game).
            java.util.function.Supplier<ClassLoader> generation = () -> new java.net.URLClassLoader(new java.net.URL[] {dir}, app) {
                @Override
                protected Class<?> loadClass(String n, boolean resolve) throws ClassNotFoundException {
                    if (!n.startsWith("t.")) return super.loadClass(n, resolve);
                    synchronized (getClassLoadingLock(n)) {
                        Class<?> c = findLoadedClass(n);
                        return c != null ? c : findClass(n);
                    }
                }
            };
            ClassLoader first = generation.get();
            ClassLoader next = generation.get();
            Class<?> script = Class.forName("t.KScript", true, first);
            Class<?> ext = java.util.Arrays.stream(script.getDeclaredClasses())
                .filter(c -> c.getSimpleName().startsWith("$ExtendPlayer")).findFirst().get();
            var of = ext.getDeclaredMethod("$of", String.class);
            of.setAccessible(true);
            Object old = of.invoke(null, "Steve");
            ext.getField("Combo").setInt(old, 4);
            ext.getField("Last").set(old, "jump");
            var fixed = ext.getField("Fixed");
            fixed.setAccessible(true);
            fixed.setInt(old, 7);
            var thing = Class.forName("t.KScript$Thing", true, first).getDeclaredConstructor(int.class);
            ((java.util.List<Object>) ext.getField("Bag").get(old)).add(thing.newInstance(9));
            var gate = new dev.yog.script.threading.tasks.TaskCompletionSource();
            var waiting = (dev.yog.script.threading.tasks.Task1<?>) Class.forName("t.KScript$K", true, first)
                .getMethod("Wait", dev.yog.script.threading.tasks.Task.class).invoke(null, gate.Task());
            // The reload: the old generation's live objects out, carried into the next.
            java.util.Set<ClassLoader> was = java.util.Set.of(first);
            var live = dev.yog.script.yog.Session.retire(was);
            dev.yog.script.yog.Session.carry(was, live, java.util.List.of(ext), java.util.List.of(next));
            gate.SetResult();
            Class<?> newExt = Class.forName(ext.getName(), true, next);
            var newOf = newExt.getDeclaredMethod("$of", String.class);
            newOf.setAccessible(true);
            Object fresh = newOf.invoke(null, "Steve");
            return waiting.Result() + " | " + newExt.getField("Combo").getInt(fresh) + " " + ext.getField("Combo").getInt(old);
        } catch (ReflectiveOperationException e) {
            throw new RuntimeException(e);
        }
    }).get()"#;
    let Some(out) = support::run(&[("K.ys", RELOADED)], java) else {
        return;
    };
    assert_eq!(out, "done 5 jump 7 1 9 | 5 4");
}

const SAVED_WAIT: &str = r#"using System.Threading.Tasks;
public static class W {
    public static string Run() => "ok";
}
extend Player {
    public i32 Done = 0;
    public saved async Task Cook(i32 n) {
        await GameTime.Delay(20);
        Done += n;
    }
}
"#;

/// A `saved async` method's wait goes with its owner's save (storage.md
/// S9): written mid-delay, read into a new owner (the world loaded
/// again), it waits what was left — not the whole delay — and goes on with
/// its arguments.
#[test]
fn a_saved_method_waits_the_rest_after_a_restart() {
    let java = r#"((java.util.function.Supplier<String>) () -> {
        try {
            Class<?> ext = java.util.Arrays.stream(t.WScript.class.getDeclaredClasses())
                .filter(c -> c.getSimpleName().startsWith("$ExtendPlayer")).findFirst().get();
            var make = ext.getDeclaredConstructor(String.class);
            make.setAccessible(true);
            var sched = dev.yog.script.threading.tasks.Scheduler.here();
            var tick = dev.yog.script.threading.tasks.Scheduler.class.getDeclaredMethod("tick");
            tick.setAccessible(true);
            Object before = make.newInstance("Steve");
            ext.getMethod("Cook", int.class).invoke(before, 3);
            for (int i = 0; i < 5; i++) tick.invoke(sched);
            var store = new java.util.TreeMap<String, byte[]>();
            ((dev.yog.script.yog.State.Saved) before).$save(store::put);
            // The world loaded again: a new owner reads it, the wait picked up.
            Object after = make.newInstance("Steve");
            ((dev.yog.script.yog.State.Saved) after).$load(store::get);
            StringBuilder seen = new StringBuilder(store.keySet().toString());
            for (int i = 1; i <= 15; i++) {
                tick.invoke(sched);
                if (i == 14 || i == 15) seen.append(" ").append(i).append(":").append(ext.getField("Done").getInt(after));
            }
            return seen.toString();
        } catch (ReflectiveOperationException e) {
            throw new RuntimeException(e);
        }
    }).get()"#;
    let Some(out) = support::run(&[("W.ys", SAVED_WAIT)], java) else {
        return;
    };
    assert_eq!(out, "[Player.Cook$wait] 14:0 15:3");
}
