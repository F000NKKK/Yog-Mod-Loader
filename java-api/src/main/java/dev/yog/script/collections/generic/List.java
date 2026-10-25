package dev.yog.script.collections.generic;

import java.util.AbstractList;
import java.util.ArrayList;
import java.util.RandomAccess;

/**
 * Items in order, as many as are added: Yog Script's {@code List<T>}.
 * Also a {@link java.util.List} over the items it holds — its own, or a
 * Java list it's a view of ({@link #view}, Yog's values' lists, not copied).
 */
public class List<T> extends AbstractList<T> implements IList<T>, IReadOnlyList<T>, RandomAccess {
    private final java.util.List<T> items;

    public List() {
        this(new ArrayList<>());
    }

    public List(IEnumerable<T> collection) {
        this();
        for (T item : collection) {
            items.add(item);
        }
    }

    private List(java.util.List<T> items) {
        this.items = items;
    }

    /** {@code items} as a Yog Script list: changes go through to it. */
    public static <T> List<T> view(java.util.List<T> items) {
        return items instanceof List<T> list ? list : new List<>(items);
    }

    // Java's list.

    @Override
    public T get(int index) {
        return items.get(index);
    }

    @Override
    public T set(int index, T value) {
        return items.set(index, value);
    }

    @Override
    public void add(int index, T value) {
        items.add(index, value);
    }

    @Override
    public T remove(int index) {
        return items.remove(index);
    }

    @Override
    public int size() {
        return items.size();
    }

    // Yog Script's.

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
        return false;
    }

    @Override
    public T get_Item(int index) {
        return items.get(index);
    }

    @Override
    public void set_Item(int index, T value) {
        items.set(index, value);
    }

    @Override
    public void Add(T item) {
        items.add(item);
    }

    public void AddRange(IEnumerable<T> collection) {
        for (T item : collection) {
            items.add(item);
        }
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

    @Override
    public int IndexOf(T item) {
        return items.indexOf(item);
    }

    @Override
    public void Insert(int index, T item) {
        items.add(index, item);
    }

    public void InsertRange(int index, IEnumerable<T> collection) {
        int at = index;
        for (T item : collection) {
            items.add(at++, item);
        }
    }

    @Override
    public void RemoveAt(int index) {
        items.remove(index);
    }

    public void RemoveRange(int index, int count) {
        items.subList(index, index + count).clear();
    }

    public int RemoveAll(dev.yog.script.Predicate<T> match) {
        int before = items.size();
        items.removeIf(match::Invoke);
        return before - items.size();
    }

    public T Find(dev.yog.script.Predicate<T> match) {
        for (T item : items) {
            if (match.Invoke(item)) {
                return item;
            }
        }
        return null;
    }

    public List<T> FindAll(dev.yog.script.Predicate<T> match) {
        List<T> found = new List<>();
        for (T item : items) {
            if (match.Invoke(item)) {
                found.Add(item);
            }
        }
        return found;
    }

    public int FindIndex(dev.yog.script.Predicate<T> match) {
        for (int i = 0; i < items.size(); i++) {
            if (match.Invoke(items.get(i))) {
                return i;
            }
        }
        return -1;
    }

    public boolean Exists(dev.yog.script.Predicate<T> match) {
        return FindIndex(match) >= 0;
    }

    public boolean TrueForAll(dev.yog.script.Predicate<T> match) {
        for (T item : items) {
            if (!match.Invoke(item)) {
                return false;
            }
        }
        return true;
    }

    public void ForEach(dev.yog.script.Action1<T> action) {
        for (T item : items) {
            action.Invoke(item);
        }
    }

    public List<T> GetRange(int index, int count) {
        return new List<>(new ArrayList<>(items.subList(index, index + count)));
    }

    public void Reverse() {
        java.util.Collections.reverse(items);
    }

    public void Sort() {
        items.sort(Order.natural());
    }

    public void Sort(dev.yog.script.Comparison<T> comparison) {
        items.sort(comparison::Invoke);
    }
}
