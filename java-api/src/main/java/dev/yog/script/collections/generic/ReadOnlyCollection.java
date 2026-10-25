package dev.yog.script.collections.generic;

import java.util.AbstractCollection;
import java.util.Collection;
import java.util.Iterator;

/** A Java collection seen as an {@link ICollection} that isn't changed through it (a dictionary's keys, its values). */
final class ReadOnlyCollection<T> extends AbstractCollection<T> implements ICollection<T> {
    private static final String READ_ONLY = "this collection is read-only";

    private final Collection<T> items;

    ReadOnlyCollection(Collection<T> items) {
        this.items = items;
    }

    @Override
    public Iterator<T> iterator() {
        return java.util.Collections.unmodifiableCollection(items).iterator();
    }

    @Override
    public int size() {
        return items.size();
    }

    @Override
    public IEnumerator<T> GetEnumerator() {
        return Enumerators.of(this::iterator);
    }

    @Override
    public int Count() {
        return items.size();
    }

    @Override
    public boolean IsReadOnly() {
        return true;
    }

    @Override
    public void Add(T item) {
        throw new UnsupportedOperationException(READ_ONLY);
    }

    @Override
    public void Clear() {
        throw new UnsupportedOperationException(READ_ONLY);
    }

    @Override
    public boolean Contains(T item) {
        return items.contains(item);
    }

    @Override
    public boolean Remove(T item) {
        throw new UnsupportedOperationException(READ_ONLY);
    }
}
