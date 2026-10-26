package dev.yog.script.threading.tasks;

import dev.yog.script.Action1;
import dev.yog.script.YsPanic;
import dev.yog.script.threading.SynchronizationContext;
import java.util.ArrayList;
import java.util.List;

/**
 * Yog Script's {@code System.Threading.Tasks.Task}: work that finishes
 * later, awaited without stopping the game. Safe across threads: what
 * waits for it goes on on the thread it waited on — a side's, at its
 * next tick, when another thread finishes it ({@code Task.Run}'s pool);
 * where it's finished, on any other thread.
 */
public class Task {
    /** What a task is: still going, done, failed. */
    private static final int RUNNING = 0;
    static final int DONE = 1;
    private static final int FAILED = 2;
    private static final Task COMPLETED = done();
    private static final double NANOS_PER_SECOND = 1e9;
    /** How often a thread blocked on a task runs what's posted to it. */
    private static final long PUMP_MILLIS = 1;

    private volatile int state = RUNNING;
    private volatile Throwable failure;
    /** Guarded by {@code this}. */
    private List<Runnable> waiting = new ArrayList<>();

    public boolean IsCompleted() {
        return state != RUNNING;
    }

    public boolean IsCompletedSuccessfully() {
        return state == DONE;
    }

    public boolean IsFaulted() {
        return state == FAILED;
    }

    public static Task CompletedTask() {
        return COMPLETED;
    }

    /** Done once {@code seconds} of real time have passed: at a tick of a side's thread, else by the pool's timer. */
    public static Task Delay(double seconds) {
        Task t = new Task();
        long nanos = Math.round(seconds * NANOS_PER_SECOND);
        Scheduler here = Scheduler.current();
        if (here != null) {
            here.afterNanos(nanos, t::finish);
        } else {
            dev.yog.script.threading.ThreadPool.later(nanos, t::finish);
        }
        return t;
    }

    public static Task Delay(dev.yog.script.TimeSpan delay) {
        return Delay(delay.TotalSeconds());
    }

    /** Done at the next turn of its side (on another thread: by the pool, at once). */
    public static Task Yield() {
        Task t = new Task();
        Scheduler here = Scheduler.current();
        if (here != null) {
            here.afterTicks(1, t::finish);
        } else {
            dev.yog.script.threading.ThreadPool.later(0, t::finish);
        }
        return t;
    }

    private static final TaskFactory FACTORY = new TaskFactory();

    /** The factory of tasks on the pool ({@code Task.Factory}). */
    public static TaskFactory Factory() {
        return FACTORY;
    }

    /** {@code work} run on the pool: its task, awaited back in the awaiting context. */
    public static <TResult> Task1<TResult> Run(dev.yog.script.Func1<TResult> work) {
        return FACTORY.StartNew(work);
    }

    /** {@code work} run on the pool: its task. */
    public static Task Run(dev.yog.script.Action work) {
        return FACTORY.StartNew(work);
    }

    public static <TResult> Task1<TResult> FromResult(TResult result) {
        Task1<TResult> t = new Task1<>();
        t.finish(result);
        return t;
    }

    /** Done when every one of {@code tasks} is (they may finish on any thread); failed as the first that fails. */
    public static Task WhenAll(Task[] tasks) {
        Task all = new Task();
        java.util.concurrent.atomic.AtomicInteger left = new java.util.concurrent.atomic.AtomicInteger(tasks.length);
        if (tasks.length == 0) {
            all.finish();
        }
        for (Task t : tasks) {
            t.onDoneUncaptured(() -> {
                if (t.IsFaulted()) {
                    all.fail(t.failureOf());
                } else if (left.decrementAndGet() == 0) {
                    all.finish();
                }
            });
        }
        return all;
    }

    /** {@code WhenAll<TResult>}: their results in an array of {@code item}s, in order. */
    public static <T> Task1<Object> WhenAll(Class<?> item, Task1<T>[] tasks) {
        Task1<Object> all = new Task1<>();
        WhenAll((Task[]) tasks).onDoneUncaptured(() -> {
            Throwable failed = firstFailure(tasks);
            if (failed != null) {
                all.fail(failed);
                return;
            }
            Object results = java.lang.reflect.Array.newInstance(item, tasks.length);
            for (int i = 0; i < tasks.length; i++) {
                java.lang.reflect.Array.set(results, i, tasks[i].Result());
            }
            all.finish(results);
        });
        return all;
    }

    /** The first of {@code tasks} to have failed's failure, if one has. */
    private static Throwable firstFailure(Task[] tasks) {
        for (Task t : tasks) {
            if (t.IsFaulted()) {
                return t.failureOf();
            }
        }
        return null;
    }

    /** {@code WhenAny<TResult>}: the first of {@code tasks} done. */
    public static <T> Task1<Task1<T>> WhenAny(Task1<T>[] tasks) {
        Task1<Task1<T>> any = new Task1<>();
        for (Task1<T> t : tasks) {
            t.onDoneUncaptured(() -> any.finish(t));
        }
        return any;
    }

    /** Why it failed ({@code null} when it didn't). */
    public Throwable failureOf() {
        return failure;
    }

    public static Task1<Task> WhenAny(Task[] tasks) {
        Task1<Task> any = new Task1<>();
        for (Task t : tasks) {
            t.onDoneUncaptured(() -> any.finish(t));
        }
        return any;
    }

    public Task ContinueWith(Action1<Task> continuation) {
        Task next = new Task();
        onDone(() -> {
            try {
                continuation.Invoke(this);
                next.finish();
            } catch (RuntimeException e) {
                next.fail(e);
            }
        });
        return next;
    }

    /**
     * {@code then} run once it's done: now, when it is; else in the
     * synchronization context of the thread asking ({@code await}'s), posted
     * there when another thread finishes it; with none, where it's finished.
     */
    public void onDone(Runnable given) {
        Runnable then = dev.yog.script.threading.ExecutionContext.flowing(given);
        SynchronizationContext home = SynchronizationContext.Current();
        Runnable resume = home == null ? then : () -> {
            if (home.IsCurrent()) {
                then.run();
            } else {
                home.Post(then::run);
            }
        };
        onDoneUncaptured(resume, then);
    }

    /** {@code then} run once it's done, where it's finished: no context taken ({@code ConfigureAwait(false)}). */
    public void onDoneUncaptured(Runnable given) {
        Runnable then = dev.yog.script.threading.ExecutionContext.flowing(given);
        onDoneUncaptured(then, then);
    }

    /** {@code later} kept for when it's done, or {@code now} run when it is. */
    private void onDoneUncaptured(Runnable later, Runnable now) {
        synchronized (this) {
            if (!IsCompleted()) {
                waiting.add(later);
                return;
            }
        }
        now.run();
    }

    /** What {@code await} uses: {@code GetResult()} waits as {@link #Wait()} does. */
    public TaskAwaiter GetAwaiter() {
        return new TaskAwaiter(this);
    }

    /** Waits for every one of {@code tasks}, blocking (as {@link #Wait()}). */
    public static void WaitAll(Task[] tasks) {
        WhenAll(tasks).Wait();
    }

    /** Waits for one of {@code tasks}, blocking: its index. */
    public static int WaitAny(Task[] tasks) {
        Task done = WhenAny(tasks).Result();
        for (int i = 0; i < tasks.length; i++) {
            if (tasks[i] == done) {
                return i;
            }
        }
        return -1;
    }

    /** As it is, or (`false`) awaited without its context: going on where it's finished. */
    public Task ConfigureAwait(boolean continueOnCapturedContext) {
        return continueOnCapturedContext ? this : new Uncaptured<Object>(this);
    }

    /**
     * Waits for it, its failure thrown. On a side's thread, what's posted to
     * it meanwhile runs (an awaiting continuation of its own): the thread
     * doesn't wait for itself.
     */
    public void Wait() {
        waitDone();
        check();
    }

    /** Waits at most {@code timeout}: whether it's done (its failure thrown when it failed). */
    public boolean Wait(dev.yog.script.TimeSpan timeout) {
        long until = System.nanoTime() + Math.round(timeout.TotalSeconds() * NANOS_PER_SECOND);
        waitUntil(until);
        if (IsCompleted()) {
            check();
        }
        return IsCompleted();
    }

    /** Blocks until it's done, pumping its side's posted work. */
    void waitDone() {
        waitUntil(Long.MAX_VALUE);
    }

    private void waitUntil(long until) {
        if (IsCompleted()) {
            return;
        }
        Scheduler side = Scheduler.current();
        java.util.concurrent.CountDownLatch done = new java.util.concurrent.CountDownLatch(1);
        onDoneUncaptured(done::countDown);
        while (!IsCompleted() && System.nanoTime() < until) {
            if (side != null) {
                side.runPosted();
            }
            try {
                done.await(PUMP_MILLIS, java.util.concurrent.TimeUnit.MILLISECONDS);
            } catch (InterruptedException e) {
                java.lang.Thread.currentThread().interrupt();
                return;
            }
        }
    }

    /** Its failure thrown again, as awaiting a failed task does. */
    public void check() {
        if (failure instanceof RuntimeException e) {
            throw e;
        }
        if (failure instanceof Error e) {
            throw e;
        }
        if (failure != null) {
            throw new YsPanic(failure.toString(), "");
        }
    }

    /** It done; what waits for it goes on. */
    public void finish() {
        settle(DONE, null);
    }

    /** It failed with {@code why}; what waits for it goes on, to fail. */
    public void fail(Throwable why) {
        settle(FAILED, why);
    }

    private void settle(int to, Throwable why) {
        settle(to, why, null);
    }

    /**
     * It settled as {@code to} (with {@code why}), {@code store} keeping its
     * result first under the same lock — the first to settle it wins, its
     * result with it; what waits is woken outside the lock.
     */
    final boolean settle(int to, Throwable why, Runnable store) {
        List<Runnable> woken;
        synchronized (this) {
            if (IsCompleted()) {
                return false;
            }
            if (store != null) {
                store.run();
            }
            failure = why;
            state = to;
            woken = waiting;
            waiting = List.of();
        }
        woken.forEach(Runnable::run);
        return true;
    }

    /** It done, unless it's settled already: whether this settled it. */
    public boolean tryFinish() {
        return settle(DONE, null, null);
    }

    /** It failed with {@code why}, unless it's settled already: whether this settled it. */
    public boolean tryFail(Throwable why) {
        return settle(FAILED, why, null);
    }

    private static Task done() {
        Task t = new Task();
        t.finish();
        return t;
    }
}
