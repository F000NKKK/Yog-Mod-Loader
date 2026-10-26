package dev.yog.script.linq;

import dev.yog.script.collections.generic.Enumerators;
import dev.yog.script.collections.generic.IEnumerator;
import java.util.Iterator;
import java.util.List;

/** One key's items. */
final class Grouping<K, T> implements IGrouping<K, T> {
    private final K key;
    private final List<T> items;

    Grouping(K key, List<T> items) {
        this.key = key;
        this.items = items;
    }

    @Override
    public K Key() {
        return key;
    }

    @Override
    public Iterator<T> iterator() {
        return items.iterator();
    }

    @Override
    public IEnumerator<T> GetEnumerator() {
        return Enumerators.of(items::iterator);
    }
}
