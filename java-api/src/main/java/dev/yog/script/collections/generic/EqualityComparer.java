package dev.yog.script.collections.generic;

/** What makes {@code T}s equal, as a value: their own {@code Equals} and {@code GetHashCode}. */
public abstract class EqualityComparer<T> implements IEqualityComparer<T> {
    public static <T> EqualityComparer<T> Default() {
        return new EqualityComparer<>() {
            @Override
            public boolean Equals(T x, T y) {
                return java.util.Objects.equals(x, y);
            }

            @Override
            public int GetHashCode(T obj) {
                return java.util.Objects.hashCode(obj);
            }
        };
    }

    @Override
    public abstract boolean Equals(T x, T y);

    @Override
    public abstract int GetHashCode(T obj);
}
