package dev.yog.script.collections.generic;

/** Items each there once, combined and compared as sets. */
public interface ISet<T> extends ICollection<T> {
    /** {@code item} added; whether it was. C#'s {@code new bool Add(T)}: Java can't hide {@code ICollection}'s, so it's named apart. */
    boolean ISet$Add(T item);

    void UnionWith(IEnumerable<T> other);

    void IntersectWith(IEnumerable<T> other);

    void ExceptWith(IEnumerable<T> other);

    void SymmetricExceptWith(IEnumerable<T> other);

    boolean IsSubsetOf(IEnumerable<T> other);

    boolean IsSupersetOf(IEnumerable<T> other);

    boolean Overlaps(IEnumerable<T> other);

    boolean SetEquals(IEnumerable<T> other);
}
