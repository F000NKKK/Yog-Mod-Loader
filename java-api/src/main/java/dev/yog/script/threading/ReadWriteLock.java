package dev.yog.script.threading;

import dev.yog.script.TypedObject;
import java.util.concurrent.locks.ReentrantReadWriteLock;

/** Yog Script's {@code System.Threading.ReadWriteLock<T>}: many readers or one writer. */
public final class ReadWriteLock<T> extends TypedObject {
    final ReentrantReadWriteLock lock = new ReentrantReadWriteLock();
    T value;

    public ReadWriteLock(T value) {
        this.value = value;
    }

    public ReadGuard<T> Read() {
        lock.readLock().lock();
        return new ReadGuard<>(this);
    }

    public WriteGuard<T> Write() {
        lock.writeLock().lock();
        return new WriteGuard<>(this);
    }

    public ReadGuard<T> TryRead() {
        return lock.readLock().tryLock() ? new ReadGuard<>(this) : null;
    }

    public WriteGuard<T> TryWrite() {
        return lock.writeLock().tryLock() ? new WriteGuard<>(this) : null;
    }
}
