package dev.yog.script.collections.generic;

import java.util.ArrayList;
import java.util.TreeMap;

/** Values by their keys, in the keys' order, each found by its index too: Yog Script's {@code SortedList<TKey, TValue>}. */
public class SortedList<K, V> extends Dictionary<K, V> {
    private static final int NOT_FOUND = -1;

    public SortedList() {
        super(new TreeMap<>(Order.natural()));
    }

    public SortedList(IComparer<K> comparer) {
        super(new TreeMap<>(comparer.comparator()));
    }

    public int IndexOfKey(K key) {
        return ContainsKey(key) ? new ArrayList<>(keySet()).indexOf(key) : NOT_FOUND;
    }

    public int IndexOfValue(V value) {
        return new ArrayList<>(values()).indexOf(value);
    }

    public K GetKeyAtIndex(int index) {
        return new ArrayList<>(keySet()).get(index);
    }

    public V GetValueAtIndex(int index) {
        return new ArrayList<>(values()).get(index);
    }

    public void RemoveAt(int index) {
        IDictionary$Remove(GetKeyAtIndex(index));
    }
}
