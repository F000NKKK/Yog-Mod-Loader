package dev.yog.script.collections.concurrent;

import dev.yog.script.Func2;
import dev.yog.script.Func3;
import dev.yog.script.TypedObject;
import dev.yog.script.collections.generic.List;
import java.util.concurrent.ConcurrentHashMap;

/** Yog Script's {@code ConcurrentDictionary<TKey, TValue>}: read and changed by many threads at once. */
public final class ConcurrentDictionary<K, V> extends TypedObject {
    private final ConcurrentHashMap<K, V> items = new ConcurrentHashMap<>();

    public ConcurrentDictionary() {}

    public boolean TryAdd(K key, V value) {
        return items.putIfAbsent(key, value) == null;
    }

    public V TryGetValue(K key) {
        return items.get(key);
    }

    public V TryRemove(K key) {
        return items.remove(key);
    }

    public V GetOrAdd(K key, Func2<K, V> make) {
        return items.computeIfAbsent(key, make::Invoke);
    }

    public V AddOrUpdate(K key, Func2<K, V> add, Func3<K, V, V> update) {
        return items.compute(key, (k, old) -> old == null ? add.Invoke(k) : update.Invoke(k, old));
    }

    public void Set(K key, V value) {
        items.put(key, value);
    }

    public boolean ContainsKey(K key) {
        return items.containsKey(key);
    }

    public int Count() {
        return items.size();
    }

    public boolean IsEmpty() {
        return items.isEmpty();
    }

    public List<K> Keys() {
        List<K> out = new List<>();
        items.keySet().forEach(out::Add);
        return out;
    }

    public List<V> Values() {
        List<V> out = new List<>();
        items.values().forEach(out::Add);
        return out;
    }

    public void Clear() {
        items.clear();
    }
}
