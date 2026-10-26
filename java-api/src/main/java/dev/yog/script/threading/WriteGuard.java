package dev.yog.script.threading;

import dev.yog.script.IDisposable;
import dev.yog.script.TypedObject;

/** Yog Script's {@code System.Threading.WriteGuard<T>}: a write of a {@link ReadWriteLock}. */
public final class WriteGuard<T> extends TypedObject implements IDisposable {
    private final ReadWriteLock<T> of;
    private boolean released;

    WriteGuard(ReadWriteLock<T> of) {
        this.of = of;
    }

    public T Value() {
        return of.value;
    }

    public void set_Value(T value) {
        of.value = value;
    }

    @Override
    public void Dispose() {
        if (!released) {
            released = true;
            of.lock.writeLock().unlock();
        }
    }
}
