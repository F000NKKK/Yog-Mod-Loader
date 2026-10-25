package dev.yog.script.collections.generic;

/** Items that can be added and taken away. */
public interface ICollection<T> extends IEnumerable<T> {
    int Count();

    boolean IsReadOnly();

    void Add(T item);

    void Clear();

    boolean Contains(T item);

    boolean Remove(T item);
}
