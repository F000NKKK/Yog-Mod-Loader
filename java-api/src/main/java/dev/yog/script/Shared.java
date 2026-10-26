package dev.yog.script;

/**
 * Yog Script's {@code System.Shared<T>}: a value with several owners. On
 * the JVM the collector owns the memory, so an owner is the reference
 * itself and a clone is another handle to one box.
 */
public final class Shared<T> extends TypedObject {
    private final T value;

    public Shared(T value) {
        this.value = value;
    }

    public T Value() {
        return value;
    }

    public Shared<T> Clone() {
        return this;
    }

    public Weak<T> Downgrade() {
        return new Weak<>(value);
    }

    public static <T> boolean Same(Shared<T> a, Shared<T> b) {
        return a == b;
    }
}
