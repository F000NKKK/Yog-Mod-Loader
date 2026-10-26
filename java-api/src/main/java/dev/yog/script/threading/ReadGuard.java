package dev.yog.script.threading;

import dev.yog.script.IDisposable;
import dev.yog.script.TypedObject;

/** Yog Script's {@code System.Threading.ReadGuard<T>}: a read of a {@link ReadWriteLock}. */
public final class ReadGuard<T> extends TypedObject implements IDisposable {
    private final ReadWriteLock<T> of;
    private boolean released;

    ReadGuard(ReadWriteLock<T> of) {
        this.of = of;
    }

    public T Value() {
        return of.value;
    }

    @Override
    public void Dispose() {
        if (!released) {
            released = true;
            of.lock.readLock().unlock();
        }
    }
}
