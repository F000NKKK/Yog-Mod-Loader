package dev.yog.script.linq;

import dev.yog.script.collections.generic.Enumerators;
import dev.yog.script.collections.generic.IEnumerable;
import dev.yog.script.collections.generic.IEnumerator;
import java.util.ArrayList;
import java.util.Iterator;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;

/** Items by their keys, in the order the keys were met. */
final class Lookup<K, T> implements ILookup<K, T> {
    private final Map<K, List<T>> groups = new LinkedHashMap<>();

    void add(K key, T item) {
        groups.computeIfAbsent(key, k -> new ArrayList<>()).add(item);
    }

    @Override
    public int Count() {
        return groups.size();
    }

    @Override
    public IEnumerable<T> get_Item(K key) {
        List<T> items = groups.getOrDefault(key, List.of());
        return Sequence.of(items::stream);
    }

    @Override
    public boolean Contains(K key) {
        return groups.containsKey(key);
    }

    @Override
    public Iterator<IGrouping<K, T>> iterator() {
        return groups.entrySet().stream()
                .<IGrouping<K, T>>map(e -> new Grouping<>(e.getKey(), e.getValue()))
                .iterator();
    }

    @Override
    public IEnumerator<IGrouping<K, T>> GetEnumerator() {
        return Enumerators.of(this::iterator);
    }
}
