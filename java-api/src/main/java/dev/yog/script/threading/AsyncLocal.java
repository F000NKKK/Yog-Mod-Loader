package dev.yog.script.threading;

import dev.yog.script.TypedObject;

/** Yog Script's {@code System.Threading.AsyncLocal<T>}: a value flowing with the logical call. */
public final class AsyncLocal<T> extends TypedObject {
    public AsyncLocal() {}

    @SuppressWarnings("unchecked")
    public T Value() {
        return (T) ExecutionContext.get(this);
    }

    public void set_Value(T value) {
        ExecutionContext.set(this, value);
    }
}
