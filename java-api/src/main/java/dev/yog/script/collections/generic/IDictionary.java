package dev.yog.script.collections.generic;

/** Values by their keys, each key there once; a collection of its pairs. */
public interface IDictionary<K, V> extends ICollection<KeyValuePair<K, V>> {
    V get_Item(K key);

    void set_Item(K key, V value);

    ICollection<K> Keys();

    ICollection<V> Values();

    void Add(K key, V value);

    boolean ContainsKey(K key);

    /** Whether {@code key} was there to take away; named apart from {@code ICollection}'s {@code Remove}, which Java's erasure can't tell it from. */
    boolean IDictionary$Remove(K key);

    V TryGetValue(K key);
}
