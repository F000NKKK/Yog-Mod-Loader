package dev.yog.script.collections.generic;

import java.util.AbstractMap;
import java.util.LinkedHashMap;
import java.util.Map;
import java.util.Set;

/**
 * Values by their keys, in the order they were added: Yog Script's
 * {@code Dictionary<TKey, TValue>}. Also a {@link java.util.Map} over the
 * entries it holds — its own, or a Java map's it's a view of ({@link #view}).
 */
public class Dictionary<K, V> extends AbstractMap<K, V>
        implements IDictionary<K, V>, IReadOnlyDictionary<K, V> {
    private final Map<K, V> items;

    public Dictionary() {
        this(new LinkedHashMap<>());
    }

    /** A dictionary over {@code items} (a sorted one's {@code TreeMap}). */
    protected Dictionary(Map<K, V> items) {
        this.items = items;
    }

    /** {@code items} as a Yog Script dictionary: changes go through to it. */
    public static <K, V> Dictionary<K, V> view(Map<K, V> items) {
        return items instanceof Dictionary<K, V> d ? d : new Dictionary<>(items);
    }

    // Java's map.

    @Override
    public Set<Map.Entry<K, V>> entrySet() {
        return items.entrySet();
    }

    @Override
    public V put(K key, V value) {
        return items.put(key, value);
    }

    // Yog Script's.

    @Override
    public IEnumerator<KeyValuePair<K, V>> GetEnumerator() {
        return Enumerators.of(() -> items.entrySet().stream()
                .map(e -> new KeyValuePair<>(e.getKey(), e.getValue()))
                .iterator());
    }

    @Override
    public int Count() {
        return items.size();
    }

    @Override
    public V get_Item(K key) {
        if (!items.containsKey(key)) {
            throw new IllegalArgumentException("the key `" + key + "` isn't in the dictionary");
        }
        return items.get(key);
    }

    @Override
    public void set_Item(K key, V value) {
        items.put(key, value);
    }

    @Override
    public ICollection<K> Keys() {
        return new ReadOnlyCollection<>(items.keySet());
    }

    @Override
    public ICollection<V> Values() {
        return new ReadOnlyCollection<>(items.values());
    }

    @Override
    public void Add(K key, V value) {
        if (!TryAdd(key, value)) {
            throw new IllegalArgumentException("the key `" + key + "` is in the dictionary already");
        }
    }

    public boolean TryAdd(K key, V value) {
        if (items.containsKey(key)) {
            return false;
        }
        items.put(key, value);
        return true;
    }

    @Override
    public boolean ContainsKey(K key) {
        return items.containsKey(key);
    }

    public boolean ContainsValue(V value) {
        return items.containsValue(value);
    }

    @Override
    public boolean IDictionary$Remove(K key) {
        if (!items.containsKey(key)) {
            return false;
        }
        items.remove(key);
        return true;
    }

    @Override
    public V TryGetValue(K key) {
        return items.get(key);
    }

    public V GetValueOrDefault(K key, V fallback) {
        return items.getOrDefault(key, fallback);
    }

    @Override
    public void Clear() {
        items.clear();
    }

    // Its pairs as a collection's: `ICollection<KeyValuePair<K, V>>`'s, explicitly.

    @Override
    public boolean IsReadOnly() {
        return false;
    }

    @Override
    public void Add(KeyValuePair<K, V> item) {
        Add(item.Key(), item.Value());
    }

    @Override
    public boolean Contains(KeyValuePair<K, V> item) {
        return items.containsKey(item.Key()) && java.util.Objects.equals(items.get(item.Key()), item.Value());
    }

    @Override
    public boolean Remove(KeyValuePair<K, V> item) {
        return Contains(item) && IDictionary$Remove(item.Key());
    }
}
