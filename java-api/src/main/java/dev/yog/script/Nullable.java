package dev.yog.script;

/**
 * Yog Script's {@code System.Nullable<T>}: what Reflection names a value
 * type's {@code T?} ({@code typeof(i32?)}). A {@code T?} is Java's boxed
 * {@code T} or {@code null}; no object of this class is made.
 */
public final class Nullable {
    private Nullable() {}
}
