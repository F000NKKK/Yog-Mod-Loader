package dev.yog.script.linq;

import dev.yog.script.collections.generic.IEnumerable;

/** Items by their keys, many to a key ({@code ToLookup}). */
public interface ILookup<K, T> extends IEnumerable<IGrouping<K, T>> {
    int Count();

    IEnumerable<T> get_Item(K key);

    boolean Contains(K key);
}
