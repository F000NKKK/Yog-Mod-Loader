package dev.yog.script;

/**
 * Yog Script's {@code System.Nullable<T>}: what Reflection names a value
 * type's {@code T?} ({@code typeof(i32?)}). A {@code T?} is Java's boxed
 * {@code T} or {@code null}, its members read off the box where they're
 * written (`nullables.rs`); no object of this class is made, its members
 * here are only its description's.
 */
public final class Nullable {
    private Nullable() {}

    public boolean HasValue() {
        throw unmade();
    }

    public Object Value() {
        throw unmade();
    }

    public Object GetValueOrDefault() {
        throw unmade();
    }

    public Object GetValueOrDefault(Object defaultValue) {
        throw unmade();
    }

    private static IllegalStateException unmade() {
        return new IllegalStateException("a `T?` is its value's box, not a `Nullable`");
    }
}
