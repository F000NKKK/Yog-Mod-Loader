package dev.yog;

import java.util.Collections;
import java.util.Set;
import java.util.WeakHashMap;
import java.util.concurrent.atomic.AtomicInteger;
import java.util.function.Supplier;

/**
 * Whose code is running: the generation of a mod's classes (its class
 * loader) that registered a handler, so a hot reload can retire the old
 * generation's handlers while the new one registers its own. Code runs as
 * its owner's: a mod's loading, and every handler it registered (what those
 * register is theirs too). Unowned: the loader's own.
 */
public final class Generations {
    private Generations() {}

    private static final ThreadLocal<Object> CURRENT = new ThreadLocal<>();
    /** How many generations were retired so far: what queues holding code
     * check to drop a retired one's ({@link #retirements()}). */
    private static final AtomicInteger RETIREMENTS = new AtomicInteger();

    /** Weak: a retired generation is remembered only while something still holds it. */
    private static final Set<Object> RETIRED = Collections.synchronizedSet(Collections.newSetFromMap(new WeakHashMap<>()));

    /** The owner of the code running now; {@code null}: the loader's own. */
    public static Object current() {
        return CURRENT.get();
    }

    /** {@code body} run as {@code owner}'s code. */
    public static <T> T within(Object owner, Supplier<T> body) {
        Object was = CURRENT.get();
        CURRENT.set(owner);
        try {
            return body.get();
        } finally {
            CURRENT.set(was);
        }
    }

    /** {@code owner} replaced by a newer generation: its bus handlers
     * unsubscribed ({@link YogBus#retire}), anything of it still queued (a
     * task's continuation) no longer run. */
    public static void retire(Object owner) {
        RETIRED.add(owner);
        YogBus.retire(owner);
        RETIREMENTS.incrementAndGet();
    }

    /** How many generations were retired so far: a queue that saw fewer has some to drop. */
    public static int retirements() {
        return RETIREMENTS.get();
    }

    /** Code queued as {@code owner}'s ({@code then}): run as its, unless it's retired by then. */
    public record Queued(Object owner, Runnable then) implements Runnable {
        @Override
        public void run() {
            if (!retired(owner)) {
                within(owner, () -> {
                    then.run();
                    return null;
                });
            }
        }

        /** Whether {@code r} is a retired generation's queued code. */
        public static boolean dropped(Object r) {
            return r instanceof Queued q && retired(q.owner());
        }
    }

    /** Whether {@code owner} (null: the loader's own, never) was retired. */
    public static boolean retired(Object owner) {
        return owner != null && RETIRED.contains(owner);
    }
}
