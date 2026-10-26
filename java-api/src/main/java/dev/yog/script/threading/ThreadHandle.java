package dev.yog.script.threading;

import dev.yog.script.Func1;
import dev.yog.script.TypedObject;

/** Yog Script's {@code System.Threading.ThreadHandle<T>}: a thread {@code Thread.Run} started. */
public final class ThreadHandle<T> extends TypedObject {
    private final java.lang.Thread thread;
    private volatile T value;
    private volatile RuntimeException failed;

    private ThreadHandle(Func1<T> work) {
        this.thread = new java.lang.Thread(() -> {
            try {
                value = work.Invoke();
            } catch (RuntimeException e) {
                failed = e;
            }
        });
    }

    static <T> ThreadHandle<T> started(Func1<T> work) {
        ThreadHandle<T> h = new ThreadHandle<>(work);
        h.thread.start();
        return h;
    }

    /** Waits for it: its value; its panic thrown again here. */
    public T Join() {
        boolean interrupted = false;
        while (true) {
            try {
                thread.join();
                break;
            } catch (InterruptedException e) {
                interrupted = true;
            }
        }
        if (interrupted) {
            java.lang.Thread.currentThread().interrupt();
        }
        if (failed != null) {
            throw failed;
        }
        return value;
    }

    public boolean IsFinished() {
        return !thread.isAlive();
    }
}
