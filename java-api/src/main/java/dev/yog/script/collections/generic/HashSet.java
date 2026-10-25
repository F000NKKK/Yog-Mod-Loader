package dev.yog.script.collections.generic;

import java.util.AbstractSet;
import java.util.Iterator;
import java.util.LinkedHashSet;
import java.util.Set;

/**
 * Items each there once: Yog Script's {@code HashSet<T>} — gone through in
 * the order they were added, so a game's runs repeat. Also a {@link java.util.Set}.
 */
public class HashSet<T> extends AbstractSet<T> implements ISet<T> {
    private final Set<T> items = new LinkedHashSet<>();

    public HashSet() {}

    public HashSet(IEnumerable<T> collection) {
        for (T item : collection) {
            items.add(item);
        }
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

    /** {@code other}'s items, each once. */
    private static <T> Set<T> copy(IEnumerable<T> other) {
        Set<T> out = new LinkedHashSet<>();
        for (T item : other) {
            out.add(item);
        }
        return out;
    }
}
