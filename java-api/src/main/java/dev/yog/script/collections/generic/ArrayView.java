package dev.yog.script.collections.generic;

import java.util.AbstractList;
import java.util.Comparator;
import java.util.RandomAccess;
import java.util.function.IntFunction;

/**
 * An array seen as an {@link IList} (a {@code T[]} is one in Yog Script):
 * its items read and written through it, its length fixed — adding or
 * taking away isn't supported, as .NET's array's.
 */
public final class ArrayView<T> extends AbstractList<T> implements IList<T>, IReadOnlyList<T>, RandomAccess {
    private static final String FIXED = "an array's length is fixed";
    private static final int NOT_FOUND = -1;

    /** Writes an item of the array. */
    @FunctionalInterface
    public interface Slot<T> {
        void set(int index, T value);
    }

    private final int length;
    private final IntFunction<T> get;
    private final Slot<T> set;

    public ArrayView(int length, IntFunction<T> get, Slot<T> set) {
        this.length = length;
        this.get = get;
        this.set = set;
    }

    /** Where {@code value} is in sorted {@code items} (the complement of where it'd go when it isn't). */
    public static <T> int binarySearch(IList<T> items, T value, Comparator<T> order) {
        int low = 0;
        int high = items.Count() - 1;
        while (low <= high) {
            int mid = (low + high) >>> 1;
            int c = order.compare(items.get_Item(mid), value);
            if (c == 0) {
                return mid;
            }
            if (c < 0) {
                low = mid + 1;
            } else {
                high = mid - 1;
            }
        }
        return ~low;
    }

    // Java's list.

    @Override
    public T get(int index) {
        java.util.Objects.checkIndex(index, length);
        return get.apply(index);
    }

    @Override
    public T set(int index, T value) {
        T was = get(index);
        set.set(index, value);
        return was;
    }

    @Override
    public int size() {
        return length;
    }

    // Yog Script's.

    @Override
    public IEnumerator<T> GetEnumerator() {
        return Enumerators.of(this::iterator);
    }

    @Override
    public int Count() {
        return length;
    }

    @Override
    public boolean IsReadOnly() {
        return false;
    }

    @Override
    public T get_Item(int index) {
        return get(index);
    }

    @Override
    public void set_Item(int index, T value) {
        set(index, value);
    }

    @Override
    public void Add(T item) {
        throw new UnsupportedOperationException(FIXED);
    }

    @Override
    public void Clear() {
        throw new UnsupportedOperationException(FIXED);
    }

    @Override
    public boolean Contains(T item) {
        return IndexOf(item) != NOT_FOUND;
    }

    @Override
    public boolean Remove(T item) {
        throw new UnsupportedOperationException(FIXED);
    }

    @Override
    public int IndexOf(T item) {
        return indexOf(item);
    }

    @Override
    public void Insert(int index, T item) {
        throw new UnsupportedOperationException(FIXED);
    }

    @Override
    public void RemoveAt(int index) {
        throw new UnsupportedOperationException(FIXED);
    }

    // What `System.Array` does through it.

    public int LastIndexOf(T item) {
        return lastIndexOf(item);
    }

    public void Sort(dev.yog.script.Comparison<T> comparison) {
        sort(comparison::Invoke);
    }

    public void Reverse() {
        java.util.Collections.reverse(this);
    }

    public boolean Exists(dev.yog.script.Predicate<T> match) {
        return FindIndex(match) != NOT_FOUND;
    }

    public T Find(dev.yog.script.Predicate<T> match) {
        int at = FindIndex(match);
        return at == NOT_FOUND ? null : get(at);
    }

    public List<T> FindAll(dev.yog.script.Predicate<T> match) {
        List<T> found = new List<>();
        for (T item : this) {
            if (match.Invoke(item)) {
                found.Add(item);
            }
        }
        return found;
    }

    public int FindIndex(dev.yog.script.Predicate<T> match) {
        for (int i = 0; i < length; i++) {
            if (match.Invoke(get(i))) {
                return i;
            }
        }
        return NOT_FOUND;
    }

    public boolean TrueForAll(dev.yog.script.Predicate<T> match) {
        return FindIndex(item -> !match.Invoke(item)) == NOT_FOUND;
    }

    public void ForEach(dev.yog.script.Action1<T> action) {
        for (T item : this) {
            action.Invoke(item);
        }
    }

    /** Each item converted, in an array of objects (made the type wanted where it's handed on). */
    public <O> Object[] ConvertAll(dev.yog.script.Converter<T, O> converter) {
        Object[] out = new Object[length];
        for (int i = 0; i < length; i++) {
            out[i] = converter.Invoke(get(i));
        }
        return out;
    }
}
