package dev.yog.script.collections.generic;

/** Items each there once, compared as sets, read only. */
public interface IReadOnlySet<T> extends IReadOnlyCollection<T> {
    boolean Contains(T item);

    boolean IsSubsetOf(IEnumerable<T> other);

    boolean IsSupersetOf(IEnumerable<T> other);

    boolean IsProperSubsetOf(IEnumerable<T> other);

    boolean IsProperSupersetOf(IEnumerable<T> other);

    boolean Overlaps(IEnumerable<T> other);

    boolean SetEquals(IEnumerable<T> other);
}
