package dev.yog.script.linq;

import dev.yog.script.collections.generic.Enumerators;
import dev.yog.script.collections.generic.IEnumerable;
import dev.yog.script.collections.generic.IEnumerator;
import java.util.Comparator;
import java.util.Iterator;

/** {@code source}'s items sorted by {@code order} when they're gone through (a stable sort, as C#'s). */
final class OrderedSequence<T> implements IOrderedEnumerable<T> {
    private final IEnumerable<T> source;
    private final Comparator<T> order;

    OrderedSequence(IEnumerable<T> source, Comparator<T> order) {
        this.source = source;
        this.order = order;
    }

    @Override
    public Comparator<T> order() {
        return order;
    }

    @Override
    public IEnumerable<T> source() {
        return source;
    }

    @Override
    public Iterator<T> iterator() {
        return Sequence.stream(source).sorted(order).iterator();
    }

    @Override
    public IEnumerator<T> GetEnumerator() {
        return Enumerators.of(this::iterator);
    }
}
