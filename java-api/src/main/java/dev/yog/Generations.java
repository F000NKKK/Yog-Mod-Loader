package dev.yog;

import java.util.Collections;
import java.util.Map;
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

    /** {@code body} run as the loader's own code: what it registers (a
     * dispatcher every mod shares, made as the first one needs it) is no
     * mod's, so no mod's reload unsubscribes it. */
    public static void unowned(Runnable body) {
        within(null, () -> {
            body.run();
            return null;
        });
    }

    /** Each retired generation's successor, once it's loaded (weak, as {@link #RETIRED}). */
    private static final Map<Object, Object> SUCCESSORS = Collections.synchronizedMap(new WeakHashMap<>());
    /** Retired generations nothing succeeds (their mod's reload failed). */
    private static final Set<Object> GONE = Collections.synchronizedSet(Collections.newSetFromMap(new WeakHashMap<>()));

    /** {@code owner} being replaced by a newer generation: its bus handlers
     * unsubscribed ({@link YogBus#retire}); what it queued waits for
     * {@link #succeed} or {@link #gone}. */
    public static void retire(Object owner) {
        RETIRED.add(owner);
        YogBus.retire(owner);
        RETIREMENTS.incrementAndGet();
    }

    /** Retired {@code owner}'s successor: what it queued runs as {@code next}'s now. */
    public static void succeed(Object owner, Object next) {
        SUCCESSORS.put(owner, next);
    }

    /** Retired {@code owner} with no successor: what it queued is dropped. */
    public static void gone(Object owner) {
        GONE.add(owner);
        RETIREMENTS.incrementAndGet();
    }

    /** Whether {@code code} (a lambda, a method's object) is a retired
     * generation's own: its code is the old classes', not carried. */
    public static boolean stale(Object code) {
        return code != null && retired(code.getClass().getClassLoader());
    }

    /** Who {@code owner}'s queued code runs as now: its latest successor;
     * {@code null} when it's gone. */
    public static Object now(Object owner) {
        Object o = owner;
        while (o != null && RETIRED.contains(o)) {
            if (GONE.contains(o)) {
                return null;
            }
            Object next = SUCCESSORS.get(o);
            if (next == null) {
                return o; // its reload still going: it runs as it was
            }
            o = next;
        }
        return o;
    }

    /** How many generations were retired or went so far: a queue that saw fewer has some to drop. */
    public static int retirements() {
        return RETIREMENTS.get();
    }

    /** Code queued as {@code owner}'s ({@code then}): run as its successor's
     * when a reload replaced it, dropped when it's {@link #gone}. */
    public record Queued(Object owner, Runnable then) implements Runnable {
        @Override
        public void run() {
            Object as = now(owner);
            // The old classes' own code isn't carried: their successor queued its own.
            if (as != null && !stale(then)) {
                within(as, () -> {
                    then.run();
                    return null;
                });
            }
        }

        /** Whether {@code r} is a gone generation's queued code. */
        public static boolean dropped(Object r) {
            return r instanceof Queued q && (now(q.owner()) == null || stale(q.then()));
        }
    }

    /** Whether {@code owner} (null: the loader's own, never) was retired. */
    public static boolean retired(Object owner) {
        return owner != null && RETIRED.contains(owner);
    }
}
