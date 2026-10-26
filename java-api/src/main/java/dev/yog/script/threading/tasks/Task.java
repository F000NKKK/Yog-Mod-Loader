package dev.yog.script.threading.tasks;

import dev.yog.script.Action1;
import dev.yog.script.YsPanic;
import java.util.ArrayList;
import java.util.List;

/**
 * Yog Script's {@code System.Threading.Tasks.Task}: work that finishes
 * later, awaited without stopping the game. What waits for it runs on the
 * thread that finishes it — its side's, at the tick it's done in.
 */
public class Task {
    /** What a task is: still going, done, failed. */
    private static final int RUNNING = 0;
    private static final int DONE = 1;
    private static final int FAILED = 2;
    private static final Task COMPLETED = done();
    private static final double NANOS_PER_SECOND = 1e9;

    private int state = RUNNING;
    private Throwable failure;
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

    /** Done once {@code seconds} of real time have passed. */
    public static Task Delay(double seconds) {
        Task t = new Task();
        Scheduler.here().afterNanos(Math.round(seconds * NANOS_PER_SECOND), t::finish);
        return t;
    }

    public static Task Delay(dev.yog.script.TimeSpan delay) {
        return Delay(delay.TotalSeconds());
    }

    /** Done at the next turn of its side. */
    public static Task Yield() {
        Task t = new Task();
        Scheduler.here().afterTicks(1, t::finish);
        return t;
    }

    public static <TResult> Task1<TResult> FromResult(TResult result) {
        Task1<TResult> t = new Task1<>();
        t.finish(result);
        return t;
    }

    public static Task WhenAll(Task[] tasks) {
        Task all = new Task();
        int[] left = {tasks.length};
        if (left[0] == 0) {
            all.finish();
        }
        for (Task t : tasks) {
            t.onDone(() -> {
                if (all.IsCompleted()) {
                    return;
                }
                if (t.IsFaulted()) {
                    all.fail(t.failure);
                } else if (--left[0] == 0) {
                    all.finish();
                }
            });
        }
        return all;
    }

    public static Task1<Task> WhenAny(Task[] tasks) {
        Task1<Task> any = new Task1<>();
        for (Task t : tasks) {
            t.onDone(() -> {
                if (!any.IsCompleted()) {
                    any.finish(t);
                }
            });
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

    /** {@code then} run once it's done: now, when it is. */
    public void onDone(Runnable then) {
        if (IsCompleted()) {
            then.run();
        } else {
            waiting.add(then);
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
        if (IsCompleted()) {
            return;
        }
        state = to;
        failure = why;
        List<Runnable> woken = waiting;
        waiting = List.of();
        woken.forEach(Runnable::run);
    }

    private static Task done() {
        Task t = new Task();
        t.finish();
        return t;
    }
}
