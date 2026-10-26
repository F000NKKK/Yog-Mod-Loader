package dev.yog.script.threading;

import java.util.Collections;
import java.util.HashMap;
import java.util.Map;

/**
 * What flows with a logical call (.NET's {@code ExecutionContext}): the
 * values of its {@link AsyncLocal}s. Taken where work is handed on (an
 * {@code await}'s continuation, a task started, a thread), set while that
 * work runs; a change made in it stays in it.
 */
public final class ExecutionContext {
    private ExecutionContext() {}

    private static final java.lang.ThreadLocal<Map<Object, Object>> CURRENT =
            java.lang.ThreadLocal.withInitial(Map::of);

    /** The values flowing here now. */
    public static Map<Object, Object> capture() {
        return CURRENT.get();
    }

    /** {@code values} flowing here from now on. */
    public static void restore(Map<Object, Object> values) {
        CURRENT.set(values);
    }

    /** {@code work}, run with the values flowing here now (nothing to carry: as it is). */
    public static Runnable flowing(Runnable work) {
        Map<Object, Object> taken = capture();
        if (taken.isEmpty()) {
            return work;
        }
        return () -> {
            Map<Object, Object> outer = capture();
            CURRENT.set(taken);
            try {
                work.run();
            } finally {
                CURRENT.set(outer);
            }
        };
    }

    static Object get(Object key) {
        return CURRENT.get().get(key);
    }

    /** {@code key} set to {@code value} here: a new map, so what took the old one keeps it. */
    static void set(Object key, Object value) {
        Map<Object, Object> next = new HashMap<>(CURRENT.get());
        if (value == null) {
            next.remove(key);
        } else {
            next.put(key, value);
        }
        CURRENT.set(Collections.unmodifiableMap(next));
    }
}
