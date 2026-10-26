package dev.yog.script.threading;

import dev.yog.script.IDisposable;
import dev.yog.script.TimeSpan;
import dev.yog.script.TypedObject;
import java.util.concurrent.TimeUnit;

/** Yog Script's {@code System.Threading.LockGuard<T>}: a held {@link Lock}, until it's dropped. */
public final class LockGuard<T> extends TypedObject implements IDisposable {
    private final Lock<T> of;
    private boolean released;

    LockGuard(Lock<T> of) {
        this.of = of;
    }

    public T Value() {
        return of.value;
    }

    public void set_Value(T value) {
        of.value = value;
    }

    public void Wait() {
        try {
            of.condition.await();
        } catch (InterruptedException e) {
            java.lang.Thread.currentThread().interrupt();
        }
    }

    public boolean Wait(TimeSpan timeout) {
        try {
            return of.condition.await(Thread.millis(timeout), TimeUnit.MILLISECONDS);
        } catch (InterruptedException e) {
            java.lang.Thread.currentThread().interrupt();
            return false;
        }
    }

    @Override
    public void Dispose() {
        if (!released) {
            released = true;
            of.lock.unlock();
        }
    }
}
