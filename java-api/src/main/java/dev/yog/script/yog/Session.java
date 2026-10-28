package dev.yog.script.yog;

import dev.yog.script.threading.tasks.Async;
import java.util.Collections;
import java.util.HashMap;
import java.util.IdentityHashMap;
import java.util.List;
import java.util.Map;
import java.util.Set;
import java.util.WeakHashMap;
import java.util.concurrent.ConcurrentHashMap;

/**
 * What a hot reload keeps (docs/design/storage.md S8, S9), for as long as
 * the game runs: every live object of a mod's old classes (its mod object,
 * an owner's {@code extend} instance, a block's or an entity's object)
 * carried into the new classes' object of the same owner — every field,
 * {@code readonly} ones too, whatever its type ({@link Rebind}); the old
 * classes' statics into the new ones'; each {@code async} method waiting
 * mid-way into its machine's new class, to go on from where it was.
 *
 * <p>A new object is made by the new code as it always is (its
 * constructor subscribing its events); what the old one held is copied in
 * as it's made live ({@link #live}) — now, for one made already (the mod's).
 */
public final class Session {
    private Session() {}

    /** Each live object's key (its mod, its class, its owner); weak: an owner gone, its object goes. */
    private static final Map<Object, String> LIVE = Collections.synchronizedMap(new WeakHashMap<>());
    /** An old object waiting for the new object of its key, and what carries it over. */
    private record Carried(Object old, Rebind rebind) {}

    private static final Map<String, Carried> CARRIED = new ConcurrentHashMap<>();
    /** What makes a class's object of an owner (an {@code extend}'s), by its keys' start: mod|class|. */
    private static final Map<String, java.util.function.Function<String, ?>> MAKERS = new ConcurrentHashMap<>();
    /** What separates a key's parts: the mod, the class, the owner. */
    private static final String KEY_SEP = dev.yog.YogNames.Storage.SESSION_KEY_SEP;

    /** The key of {@code owner}'s object of {@code kind} (a block's, an entity's): {@code kind|owner}. */
    public static String key(String kind, String owner) {
        return kind + KEY_SEP + owner;
    }

    /** Keys starting {@code prefix} (mod|class|) name objects {@code make} makes, by owner. */
    public static void maker(String prefix, java.util.function.Function<String, ?> make) {
        MAKERS.put(prefix, make);
    }

    /** {@code made} live under {@code key}: what an older generation's object of that key held, its now. */
    public static <T> T live(T made, String key) {
        Carried c = CARRIED.remove(key);
        if (c != null) {
            c.rebind().fields(c.old(), made);
        }
        LIVE.put(made, key);
        return made;
    }

    /** The live objects of classes {@code loaders} loaded (a generation a
     * reload replaces), by key: no longer live. */
    public static Map<String, Object> retire(Set<ClassLoader> loaders) {
        MAKERS.values().removeIf(m -> loaders.contains(m.getClass().getClassLoader()));
        Map<String, Object> old = new HashMap<>();
        synchronized (LIVE) {
            LIVE.entrySet().removeIf(e -> {
                if (!loaders.contains(e.getKey().getClass().getClassLoader())) {
                    return false;
                }
                old.put(e.getValue(), e.getKey());
                return true;
            });
        }
        return old;
    }

    /**
     * Everything of the old generation ({@code old}: its loaders, {@code
     * live}: its live objects by key, {@code classes}: what it loaded)
     * carried into the new one ({@code next}): a live object into the new
     * object of its key (now when there's one, else as it's made), the
     * statics, the async methods waiting.
     */
    public static void carry(Set<ClassLoader> old, Map<String, Object> live, List<Class<?>> classes, List<ClassLoader> next) {
        Map<String, Object> fresh = new HashMap<>();
        synchronized (LIVE) {
            LIVE.forEach((o, key) -> {
                if (next.contains(o.getClass().getClassLoader())) {
                    fresh.put(key, o);
                }
            });
        }
        Map<Object, String> keyOf = new IdentityHashMap<>();
        live.forEach((key, o) -> keyOf.put(o, key));
        // An old object some other holds: the new one of its key, once it's made (else the new value stays).
        Rebind rebind = new Rebind(old, next, o -> {
            String key = keyOf.get(o);
            if (key == null) {
                return null;
            }
            Object now = liveOf(key);
            return now != null ? now : Rebind.MISSING;
        });
        live.forEach((key, o) -> {
            Object into = fresh.get(key);
            if (into != null) {
                rebind.fields(o, into);
            } else {
                CARRIED.put(key, new Carried(o, rebind));
            }
        });
        // An owner's instance made now by its class (taking what's carried), so what refers to it finds it.
        for (String key : List.copyOf(CARRIED.keySet())) {
            int at = key.lastIndexOf(KEY_SEP);
            var make = at < 0 ? null : MAKERS.get(key.substring(0, at + 1));
            if (make != null && !old.contains(make.getClass().getClassLoader())) {
                make.apply(key.substring(at + 1));
            }
        }
        for (Class<?> was : classes) {
            Class<?> now = rebind.newClass(was.getName());
            if (now != null) {
                rebind.statics(was, now);
            }
        }
        var moved = Async.carry(m -> old.contains(m.getClass().getClassLoader()), rebind::value);
        SavedTasks.carried(moved, rebind::value);
    }

    /** The live object of {@code key}, if any. */
    private static Object liveOf(String key) {
        synchronized (LIVE) {
            for (Map.Entry<Object, String> e : LIVE.entrySet()) {
                if (e.getValue().equals(key)) {
                    return e.getKey();
                }
            }
        }
        return null;
    }

    /** What's waiting to be carried, dropped (a failed reload; the world stopping, {@code YsMods}): it keeps old classes. */
    public static void forgetCarried() {
        CARRIED.clear();
    }
}
