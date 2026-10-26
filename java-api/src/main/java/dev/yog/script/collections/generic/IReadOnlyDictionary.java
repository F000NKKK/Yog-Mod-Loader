package dev.yog.script.collections.generic;

/** Values by their keys, read only: what a dictionary is to those that mustn't change it. */
public interface IReadOnlyDictionary<K, V> extends IReadOnlyCollection<KeyValuePair<K, V>> {
    V get_Item(K key);

    IEnumerable<K> Keys();

    IEnumerable<V> Values();

    boolean ContainsKey(K key);

    V TryGetValue(K key);
}
