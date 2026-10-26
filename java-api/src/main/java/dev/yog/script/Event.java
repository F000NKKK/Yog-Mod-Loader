package dev.yog.script;

import java.util.ArrayList;
import java.util.List;
import java.util.Objects;

/**
 * An {@code event} of Yog Script, as C#'s: the handlers of delegate type
 * {@code D} added to it, each by a key ({@code -=} removes the last added
 * by an equal one — a method's, the same method of the same object; a
 * lambda's, itself). Raising it calls each, in the order they were added:
 * the compiler writes a loop over {@link #handlers()} (its arguments
 * computed once, nothing allocated), as the runtime's own raisers do.
 */
public final class Event<D> {
    /** The handlers' keys and the handlers, index for index; replaced, never changed, as handlers come and go. */
    private List<Object> keys = List.of();
    private List<D> handlers = List.of();

    /** {@code e += handler}: a lambda or a delegate's value, its own key. */
    public void add(D handler) {
        add(handler, handler);
    }

    /** {@code e += Method}: removed by {@code key} (the method and its object). */
    public synchronized void add(Object key, D handler) {
        List<Object> k = new ArrayList<>(keys);
        List<D> h = new ArrayList<>(handlers);
        k.add(key);
        h.add(handler);
        keys = List.copyOf(k);
        handlers = List.copyOf(h);
    }

    /** {@code e -= handler}: the last one added by an equal key gone. */
    public synchronized void remove(Object key) {
        for (int i = keys.size() - 1; i >= 0; i--) {
            if (Objects.equals(keys.get(i), key)) {
                List<Object> k = new ArrayList<>(keys);
                List<D> h = new ArrayList<>(handlers);
                k.remove(i);
                h.remove(i);
                keys = List.copyOf(k);
                handlers = List.copyOf(h);
                return;
            }
        }
    }

    /** The handlers as they are now: those added while they're called wait for the next raise. */
    public List<D> handlers() {
        return handlers;
    }
}
