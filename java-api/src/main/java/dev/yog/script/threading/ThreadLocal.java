package dev.yog.script.threading;

import dev.yog.script.Func1;
import dev.yog.script.TypedObject;

/** Yog Script's {@code System.Threading.ThreadLocal<T>}: a value per thread. */
public final class ThreadLocal<T> extends TypedObject {
    private final java.lang.ThreadLocal<T> values;
    private final java.lang.ThreadLocal<Boolean> made = java.lang.ThreadLocal.withInitial(() -> false);

    public ThreadLocal(Func1<T> make) {
        this.values = java.lang.ThreadLocal.withInitial(() -> {
            made.set(true);
            return make.Invoke();
        });
    }

    public T Value() {
        return values.get();
    }

    public void set_Value(T value) {
        made.set(true);
        values.set(value);
    }

    public boolean IsValueCreated() {
        return made.get();
    }
}
