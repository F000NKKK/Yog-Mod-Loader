package dev.yog.script.collections.generic;

/** An order of {@code T}s: less than zero when {@code x} goes before {@code y}. */
public interface IComparer<T> {
    int Compare(T x, T y);

    /** As Java's comparator. */
    default java.util.Comparator<T> comparator() {
        return this::Compare;
    }
}
