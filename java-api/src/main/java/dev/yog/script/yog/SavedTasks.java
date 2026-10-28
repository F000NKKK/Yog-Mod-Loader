package dev.yog.script.yog;

import dev.yog.script.threading.tasks.Async;
import java.util.Collections;
import java.util.Map;
import java.util.WeakHashMap;
import java.util.function.BiConsumer;
import java.util.function.Function;

/**
 * {@code saved async} methods running (docs/design/storage.md S9): each
 * one's machine by its owner and name, so the owner's save writes where it
 * waits (a record {@code Owner.Name$wait}) and its load picks it up again.
 */
public final class SavedTasks {
    private SavedTasks() {}

    /** What empties a method's record: it waits no more. */
    private static final byte[] NONE = new byte[0];

    /** A running method's owner and name. */
    private record Of(Object owner, String name) {}

    /** Each running machine's owner and name; weak: a machine done and dropped goes. */
    private static final Map<Async<?>, Of> RUNNING = Collections.synchronizedMap(new WeakHashMap<>());

    /** {@code machine} is {@code owner}'s method {@code name}, started (or picked up again). */
    public static <M extends Async<?>> M started(Object owner, String name, M machine) {
        synchronized (RUNNING) {
            RUNNING.entrySet().removeIf(e -> e.getValue().owner() == owner && e.getValue().name().equals(name));
            RUNNING.put(machine, new Of(owner, name));
        }
        return machine;
    }

    /** {@code owner}'s method {@code name}'s wait written under {@code key}: emptied when it waits for no delay. */
    public static void save(Object owner, String name, String key, BiConsumer<String, byte[]> write) {
        Async<?> m = running(owner, name);
        byte[] kept = m == null || m.$done() ? null : m.$kept();
        write.accept(key, kept == null ? NONE : kept);
    }

    /** What {@code read} keeps under {@code key}, when it's a wait to pick up again. */
    public static byte[] waiting(State.Records read, String key) {
        byte[] b = read.read(key);
        return b == null || b.length == 0 ? null : b;
    }

    /** {@code owner}'s method {@code name}'s saved wait wasn't picked up: its
     * code changed since the save (or the record is unreadable). It starts no more. */
    public static void lost(Object owner, String name) {
        dev.yog.YogLog.warn("Yog Script: " + owner.getClass().getName() + "." + name
                + "'s saved wait isn't picked up: its code changed since the world was saved");
    }

    /** Machines a hot reload moved ({@code old → new}) with their owners as the new classes have them. */
    public static void carried(Map<Async<?>, Async<?>> moved, Function<Object, Object> remake) {
        moved.forEach((old, next) -> {
            Of of = RUNNING.remove(old);
            if (of != null) {
                RUNNING.put(next, new Of(remake.apply(of.owner()), of.name()));
            }
        });
    }

    private static Async<?> running(Object owner, String name) {
        synchronized (RUNNING) {
            for (Map.Entry<Async<?>, Of> e : RUNNING.entrySet()) {
                if (e.getValue().owner() == owner && e.getValue().name().equals(name)) {
                    return e.getKey();
                }
            }
        }
        return null;
    }
}
