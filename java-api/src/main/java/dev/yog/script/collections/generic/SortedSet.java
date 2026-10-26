package dev.yog.script.collections.generic;

import java.util.AbstractSet;
import java.util.Iterator;
import java.util.NavigableSet;
import java.util.NoSuchElementException;
import java.util.TreeSet;

/** Items each there once, in their order: Yog Script's {@code SortedSet<T>}. */
public class SortedSet<T> extends AbstractSet<T> implements ISet<T>, IReadOnlySet<T> {
    private static final String EMPTY = "the set is empty";

    private final NavigableSet<T> items;

    public SortedSet() {
        this(new TreeSet<>(Order.natural()));
    }

    public SortedSet(IComparer<T> comparer) {
        this(new TreeSet<>(comparer.comparator()));
    }

    public SortedSet(IEnumerable<T> collection) {
        this(new TreeSet<>(Order.natural()));
        for (T item : collection) {
            items.add(item);
        }
    }

    private SortedSet(NavigableSet<T> items) {
        this.items = items;
    }

    // Java's set.

    @Override
    public Iterator<T> iterator() {
        return items.iterator();
    }

    @Override
    public int size() {
        return items.size();
    }

    @Override
    public boolean add(T item) {
        return items.add(item);
    }

    // Yog Script's.

    @Override
    public IEnumerator<T> GetEnumerator() {
        return Enumerators.of(items::iterator);
    }

    @Override
    public int Count() {
        return items.size();
    }

    @Override
    public boolean IsReadOnly() {
        return false;
    }

    public T Min() {
        try {
            return items.first();
        } catch (NoSuchElementException e) {
            throw new IllegalStateException(EMPTY, e);
        }
    }

    public T Max() {
        try {
            return items.last();
        } catch (NoSuchElementException e) {
            throw new IllegalStateException(EMPTY, e);
        }
    }

    @Override
    public boolean ISet$Add(T item) {
        return items.add(item);
    }

    /** {@code ICollection<T>.Add}, explicitly. */
    @Override
    public void Add(T item) {
        items.add(item);
    }

    @Override
    public void Clear() {
        items.clear();
    }

    @Override
    public boolean Contains(T item) {
        return items.contains(item);
    }

    @Override
    public boolean Remove(T item) {
        return items.remove(item);
    }

    public int RemoveWhere(dev.yog.script.Predicate<T> match) {
        int before = items.size();
        items.removeIf(match::Invoke);
        return before - items.size();
    }

    public IEnumerable<T> Reverse() {
        return Enumerators.enumerable(items::descendingIterator);
    }

    public SortedSet<T> GetViewBetween(T lowerValue, T upperValue) {
        return new SortedSet<>(items.subSet(lowerValue, true, upperValue, true));
    }

    @Override
    public void UnionWith(IEnumerable<T> other) {
        for (T item : other) {
            items.add(item);
        }
    }

    @Override
    public void IntersectWith(IEnumerable<T> other) {
        items.retainAll(copy(other));
    }

    @Override
    public void ExceptWith(IEnumerable<T> other) {
        for (T item : other) {
            items.remove(item);
        }
    }

    @Override
    public void SymmetricExceptWith(IEnumerable<T> other) {
        for (T item : copy(other)) {
            if (!items.remove(item)) {
                items.add(item);
            }
        }
    }

    @Override
    public boolean IsSubsetOf(IEnumerable<T> other) {
        return copy(other).containsAll(items);
    }

    @Override
    public boolean IsSupersetOf(IEnumerable<T> other) {
        return items.containsAll(copy(other));
    }

    @Override
    public boolean IsProperSubsetOf(IEnumerable<T> other) {
        return IsSubsetOf(other) && !SetEquals(other);
    }

    @Override
    public boolean IsProperSupersetOf(IEnumerable<T> other) {
        return IsSupersetOf(other) && !SetEquals(other);
    }

    @Override
    public boolean Overlaps(IEnumerable<T> other) {
        for (T item : other) {
            if (items.contains(item)) {
                return true;
            }
        }
        return false;
    }

    @Override
    public boolean SetEquals(IEnumerable<T> other) {
        return items.equals(copy(other));
    }

    /** {@code other}'s items, each once, in this set's order. */
    private java.util.Set<T> copy(IEnumerable<T> other) {
        java.util.Set<T> out = new TreeSet<>(items.comparator());
        for (T item : other) {
            out.add(item);
        }
        return out;
    }
}
