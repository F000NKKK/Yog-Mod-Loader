package dev.yog.script;

import java.lang.invoke.MethodHandles;
import java.lang.invoke.VarHandle;

/** Yog Script's {@code System.Once<T>}: a value set once, by whoever comes first. */
public final class Once<T> extends TypedObject {
    /** What it holds before it's set (a set value may be {@code null}). */
    private static final Object UNSET = new Object();
    private static final VarHandle VALUE;

    static {
        try {
            VALUE = MethodHandles.lookup().findVarHandle(Once.class, "value", Object.class);
        } catch (ReflectiveOperationException e) {
            throw new ExceptionInInitializerError(e);
        }
    }

    @SuppressWarnings("unused") // Reached through VALUE.
    private volatile Object value = UNSET;

    public boolean TrySet(T value) {
        return VALUE.compareAndSet(this, UNSET, value);
    }

    @SuppressWarnings("unchecked")
    public T GetOrInit(Func1<T> make) {
        Object v = VALUE.getAcquire(this);
        if (v != UNSET) {
            return (T) v;
        }
        synchronized (this) {
            v = VALUE.getAcquire(this);
            if (v == UNSET) {
                v = make.Invoke();
                VALUE.setRelease(this, v);
            }
        }
        return (T) v;
    }

    @SuppressWarnings("unchecked")
    public T Get() {
        Object v = VALUE.getAcquire(this);
        return v == UNSET ? null : (T) v;
    }

    public boolean IsSet() {
        return VALUE.getAcquire(this) != UNSET;
    }
}
