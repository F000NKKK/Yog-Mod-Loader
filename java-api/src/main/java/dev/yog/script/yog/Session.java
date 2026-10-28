package dev.yog.script.yog;

import java.util.Collections;
import java.util.HashMap;
import java.util.Map;
import java.util.WeakHashMap;
import java.util.concurrent.ConcurrentHashMap;
import java.util.function.BiConsumer;

/**
 * What a hot reload keeps of live objects (docs/design/storage.md S8):
 * their members that aren't {@code saved} — those are in their stores —
 * written as a mod's old classes go ({@link #retire}) and read into the new
 * classes' object of the same owner as it's made live ({@link #live}). Only
 * for as long as the game runs: the session's scope.
 */
public final class Session {
    private Session() {}

    /** An object a hot reload keeps the members of. */
    public interface Kept {
        /** Each member written, by its name. */
        void $keep(BiConsumer<String, byte[]> write);

        /** Each member read back from what its old object kept. */
        void $restore(State.Records read);
    }

    /** Each live object's key (its mod, its class, its owner); weak: an owner gone, its object goes. */
    private static final Map<Kept, String> LIVE = Collections.synchronizedMap(new WeakHashMap<>());
    /** What retired objects kept, by key, until the new object of that key takes it. */
    private static final Map<String, Map<String, byte[]>> KEPT = new ConcurrentHashMap<>();

    /** {@code made} live under {@code key}: what an older object of that key kept read in. */
    public static <T extends Kept> T live(T made, String key) {
        Map<String, byte[]> kept = KEPT.remove(key);
        if (kept != null) {
            made.$restore(kept::get);
        }
        LIVE.put(made, key);
        return made;
    }

    /** The objects of classes {@code loader} loaded (a generation a hot
     * reload replaces): their members kept by key, no longer live. */
    public static void retire(ClassLoader loader) {
        synchronized (LIVE) {
            LIVE.entrySet().removeIf(e -> {
                if (e.getKey().getClass().getClassLoader() != loader) {
                    return false;
                }
                Map<String, byte[]> kept = new HashMap<>();
                e.getKey().$keep(kept::put);
                KEPT.put(e.getValue(), kept);
                return true;
            });
        }
    }
}
