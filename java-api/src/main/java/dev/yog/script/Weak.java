package dev.yog.script;

import java.lang.ref.WeakReference;

/** Yog Script's {@code System.Weak<T>}: a reference that doesn't keep its value. */
public final class Weak<T> extends TypedObject {
    private final WeakReference<T> ref;

    public Weak(T value) {
        this.ref = new WeakReference<>(value);
    }

    public T TryGet() {
        return ref.get();
    }
}
