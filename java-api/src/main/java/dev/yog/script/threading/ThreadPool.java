package dev.yog.script.threading;

import dev.yog.script.Action;
import java.util.concurrent.Executors;
import java.util.concurrent.ScheduledExecutorService;
import java.util.concurrent.TimeUnit;

/**
 * Yog Script's {@code System.Threading.ThreadPool}: the threads work runs
 * on off the game's ({@code Task.Run}, {@code Parallel}), daemons, as many
 * as the processors; and the timer of what waits off a side's thread.
 */
public final class ThreadPool {
    private ThreadPool() {}

    private static final int THREADS = Math.max(1, Runtime.getRuntime().availableProcessors());

    private static final ScheduledExecutorService POOL = Executors.newScheduledThreadPool(THREADS, r -> {
        java.lang.Thread t = new java.lang.Thread(r, "yog-pool");
        t.setDaemon(true);
        return t;
    });

    /** {@code work} queued to run on the pool. */
    public static void QueueUserWorkItem(Action work) {
        queue(work::Invoke);
    }

    /** How many threads the pool runs work on. */
    public static int ThreadCount() {
        return THREADS;
    }

    /** {@code work} run on the pool. */
    public static void queue(Runnable work) {
        POOL.execute(work);
    }

    /** {@code then} run on the pool once {@code nanos} have passed. */
    public static void later(long nanos, Runnable then) {
        POOL.schedule(then, nanos, TimeUnit.NANOSECONDS);
    }
}
