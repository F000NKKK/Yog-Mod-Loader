package dev.yog.script.collections.generic;

import java.util.Comparator;

/** Items' own order: Yog Script's {@code IComparable<T>}, else Java's {@link Comparable}. */
public final class Order {
    private Order() {}

    @SuppressWarnings("unchecked")
    public static <T> Comparator<T> natural() {
        return (a, b) -> {
            if (a instanceof dev.yog.script.IComparable<?> c) {
                return ((dev.yog.script.IComparable<T>) c).CompareTo(b);
            }
            return ((Comparable<T>) a).compareTo(b);
        };
    }
}
