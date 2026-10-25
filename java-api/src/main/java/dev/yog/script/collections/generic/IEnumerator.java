package dev.yog.script.collections.generic;

/** A walk through an {@link IEnumerable}: {@code MoveNext} steps, {@code Current} is the item it's on. */
public interface IEnumerator<T> extends dev.yog.script.IDisposable {
    T Current();

    boolean MoveNext();

    void Reset();

    @Override
    default void Dispose() {}
}
