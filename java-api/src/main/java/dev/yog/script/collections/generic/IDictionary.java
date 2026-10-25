package dev.yog.script.collections.generic;

/** Values by their keys, each key there once; gone through as its pairs. */
public interface IDictionary<K, V> extends IReadOnlyCollection<KeyValuePair<K, V>> {
    V get_Item(K key);

    void set_Item(K key, V value);

    ICollection<K> Keys();

    ICollection<V> Values();

    void Add(K key, V value);

    boolean ContainsKey(K key);

    boolean Remove(K key);

    V TryGetValue(K key);

    void Clear();
}
