package dev.yog.script.linq;

import dev.yog.script.collections.generic.Enumerators;
import dev.yog.script.collections.generic.IEnumerable;
import dev.yog.script.collections.generic.IEnumerator;
import java.util.Iterator;
import java.util.function.Supplier;
import java.util.stream.Stream;
import java.util.stream.StreamSupport;

/**
 * Items made when they're gone through, every time anew: what LINQ gives
 * (C#'s deferred sequences — no Java stream's one pass).
 */
final class Sequence<T> implements IEnumerable<T> {
    private final Supplier<Iterator<T>> items;

    private Sequence(Supplier<Iterator<T>> items) {
        this.items = items;
    }

    /** The items {@code stream} makes, made anew each time they're gone through. */
    static <T> IEnumerable<T> of(Supplier<Stream<T>> stream) {
        return new Sequence<>(() -> stream.get().iterator());
    }

    /** {@code source}'s items as a stream, for one pass. */
    static <T> Stream<T> stream(IEnumerable<T> source) {
        return StreamSupport.stream(source.spliterator(), false);
    }

    @Override
    public Iterator<T> iterator() {
        return items.get();
    }

    @Override
    public IEnumerator<T> GetEnumerator() {
        return Enumerators.of(items);
    }
}
