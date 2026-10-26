package dev.yog.script.threading;

import dev.yog.script.TimeSpan;
import dev.yog.script.TypedObject;
import java.util.concurrent.TimeUnit;
import java.util.concurrent.locks.Condition;
import java.util.concurrent.locks.ReentrantLock;

/** Yog Script's {@code System.Threading.Lock<T>}: a value one thread at a time reaches. */
public final class Lock<T> extends TypedObject {
    final ReentrantLock lock = new ReentrantLock();
    final Condition condition = lock.newCondition();
    T value;

    public Lock(T value) {
        this.value = value;
    }

    public LockGuard<T> Enter() {
        lock.lock();
        return new LockGuard<>(this);
    }

    public LockGuard<T> TryEnter() {
        return lock.tryLock() ? new LockGuard<>(this) : null;
    }

    public LockGuard<T> TryEnter(TimeSpan timeout) {
        try {
            return lock.tryLock(Thread.millis(timeout), TimeUnit.MILLISECONDS) ? new LockGuard<>(this) : null;
        } catch (InterruptedException e) {
            java.lang.Thread.currentThread().interrupt();
            return null;
        }
    }

    public void Notify() {
        held(condition::signal);
    }

    public void NotifyAll() {
        held(condition::signalAll);
    }

    /** {@code r} run holding the lock (taken for it when this thread doesn't hold it). */
    private void held(Runnable r) {
        if (lock.isHeldByCurrentThread()) {
            r.run();
            return;
        }
        lock.lock();
        try {
            r.run();
        } finally {
            lock.unlock();
        }
    }
}
