package dev.yog.script.threading.tasks;

/** Yog Script's {@code TaskAwaiter<TResult>}: what awaits a {@link Task1}, its result. */
public final class TaskAwaiter1<TResult> extends TaskAwaiter {
    /** The task again, as one giving a result. */
    private final Task1<TResult> giving;

    TaskAwaiter1(Task1<TResult> task) {
        super(task);
        this.giving = task;
    }

    /** Its {@code new GetResult()} (named after its type: it hides {@code TaskAwaiter}'s). */
    public TResult TaskAwaiter$GetResult() {
        return giving.Result();
    }
}
