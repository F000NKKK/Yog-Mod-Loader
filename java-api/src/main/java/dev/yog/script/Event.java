package dev.yog.script;

import java.util.ArrayList;
import java.util.List;
import java.util.Objects;
import java.util.function.Consumer;
import java.util.function.Function;

/**
 * An {@code event} of Yog Script, as C#'s: the handlers of delegate type
 * {@code D} added to it, each by a key ({@code -=} removes the last added
 * by an equal one — a method's, the same method of the same object; a
 * lambda's, itself). Raising it calls each, in the order they were added.
 */
public final class Event<D> {
    /** A handler and what removes it. */
    private record Handler<D>(Object key, D delegate) {}

    private List<Handler<D>> handlers = List.of();

    /** {@code e += handler}: a lambda or a delegate's value, its own key. */
    public void add(D handler) {
        add(handler, handler);
    }

    /** {@code e += Method}: removed by {@code key} (the method and its object). */
    public synchronized void add(Object key, D handler) {
        List<Handler<D>> more = new ArrayList<>(handlers);
        more.add(new Handler<>(key, handler));
        handlers = List.copyOf(more);
    }

    /** {@code e -= handler}: the last one added by an equal key gone. */
    public synchronized void remove(Object key) {
        for (int i = handlers.size() - 1; i >= 0; i--) {
            if (Objects.equals(handlers.get(i).key(), key)) {
                List<Handler<D>> fewer = new ArrayList<>(handlers);
                fewer.remove(i);
                handlers = List.copyOf(fewer);
                return;
            }
        }
    }

    /** Raised: {@code call} given each handler (those added while it runs wait for the next). */
    public void each(Consumer<D> call) {
        for (Handler<D> h : handlers) {
            call.accept(h.delegate());
        }
    }

    /** Raised, giving what the last handler gives ({@code null} with none), as C#'s. */
    public <R> R last(Function<D, R> call) {
        R out = null;
        for (Handler<D> h : handlers) {
            out = call.apply(h.delegate());
        }
        return out;
    }
}
