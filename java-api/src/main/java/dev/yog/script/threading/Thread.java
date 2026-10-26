package dev.yog.script.threading;

import dev.yog.script.Action;
import dev.yog.script.Action1;
import dev.yog.script.Func1;
import dev.yog.script.TimeSpan;

/** Yog Script's {@code System.Threading.Thread}: a thread of its own. */
public final class Thread {
    /** A tick of {@link TimeSpan}'s in nanoseconds. */
    private static final long NANOS_PER_TICK = 100;
    private static final long NANOS_PER_MILLI = 1_000_000;

    /** Each thread's own, made the first time it's asked for. */
    private static final java.lang.ThreadLocal<Thread> CURRENT =
            java.lang.ThreadLocal.withInitial(() -> new Thread(java.lang.Thread.currentThread()));

    private final java.lang.Thread thread;
    private ThreadPriority priority = ThreadPriority.Normal;

    public Thread(Action start) {
        this.thread = new java.lang.Thread(start::Invoke);
    }

    private Thread(java.lang.Thread thread) {
        this.thread = thread;
    }

    public void Start() {
        thread.start();
    }

    public void Join() {
        try {
            thread.join();
        } catch (InterruptedException e) {
            java.lang.Thread.currentThread().interrupt();
        }
    }

    public boolean Join(TimeSpan timeout) {
        try {
            thread.join(millis(timeout), nanosLeft(timeout));
        } catch (InterruptedException e) {
            java.lang.Thread.currentThread().interrupt();
        }
        return !thread.isAlive();
    }

    public String Name() {
        return thread.getName();
    }

    public void set_Name(String name) {
        thread.setName(name);
    }

    public boolean IsBackground() {
        return thread.isDaemon();
    }

    public void set_IsBackground(boolean background) {
        thread.setDaemon(background);
    }

    public ThreadPriority Priority() {
        return priority;
    }

    public void set_Priority(ThreadPriority priority) {
        this.priority = priority;
        thread.setPriority(javaPriority(priority));
    }

    public boolean IsAlive() {
        return thread.isAlive();
    }

    public int ManagedThreadId() {
        return (int) thread.getId();
    }

    public static Thread CurrentThread() {
        return CURRENT.get();
    }

    public static void Sleep(TimeSpan time) {
        try {
            java.lang.Thread.sleep(millis(time), nanosLeft(time));
        } catch (InterruptedException e) {
            java.lang.Thread.currentThread().interrupt();
        }
    }

    public static void Yield() {
        java.lang.Thread.yield();
    }

    public static void SpinWait(int iterations) {
        for (int i = 0; i < iterations; i++) {
            java.lang.Thread.onSpinWait();
        }
    }

    public static <T> ThreadHandle<T> Run(Func1<T> work) {
        return ThreadHandle.started(work);
    }

    public static void Scope(Action1<ThreadScope> body) {
        ThreadScope scope = new ThreadScope();
        try {
            body.Invoke(scope);
        } finally {
            scope.joinAll();
        }
    }

    /** {@code t}'s whole milliseconds. */
    static long millis(TimeSpan t) {
        return t.Ticks * NANOS_PER_TICK / NANOS_PER_MILLI;
    }

    /** {@code t}'s nanoseconds past its whole milliseconds. */
    static int nanosLeft(TimeSpan t) {
        return (int) (t.Ticks * NANOS_PER_TICK % NANOS_PER_MILLI);
    }

    /** {@code p} as Java's 1–10. */
    private static int javaPriority(ThreadPriority p) {
        if (p == ThreadPriority.Lowest) return java.lang.Thread.MIN_PRIORITY;
        if (p == ThreadPriority.BelowNormal) return (java.lang.Thread.MIN_PRIORITY + java.lang.Thread.NORM_PRIORITY) / 2;
        if (p == ThreadPriority.AboveNormal) return (java.lang.Thread.NORM_PRIORITY + java.lang.Thread.MAX_PRIORITY) / 2;
        if (p == ThreadPriority.Highest) return java.lang.Thread.MAX_PRIORITY;
        return java.lang.Thread.NORM_PRIORITY;
    }
}
