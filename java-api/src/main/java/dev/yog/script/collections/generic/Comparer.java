package dev.yog.script.collections.generic;

/** An order of {@code T}s as a value: their own ({@code Default}), or a comparison's ({@code Create}). */
public abstract class Comparer<T> implements IComparer<T> {
    public static <T> Comparer<T> Default() {
        return Create(Order.<T>natural()::compare);
    }

    public static <T> Comparer<T> Create(dev.yog.script.Comparison<T> comparison) {
        return new Comparer<>() {
            @Override
            public int Compare(T x, T y) {
                return comparison.Invoke(x, y);
            }
        };
    }

    @Override
    public abstract int Compare(T x, T y);
}
