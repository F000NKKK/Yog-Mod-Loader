package dev.yog.script.collections.generic;

import java.util.TreeMap;

/** Values by their keys, in the keys' order: Yog Script's {@code SortedDictionary<TKey, TValue>}. */
public class SortedDictionary<K, V> extends Dictionary<K, V> {
    public SortedDictionary() {
        super(new TreeMap<>(Order.natural()));
    }

    public SortedDictionary(IComparer<K> comparer) {
        super(new TreeMap<>(comparer.comparator()));
    }
}
