package dev.yog.script.collections.generic;

/** What makes two {@code T}s equal, and the number equal ones share. */
public interface IEqualityComparer<T> {
    boolean Equals(T x, T y);

    int GetHashCode(T obj);
}
