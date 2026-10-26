package dev.yog.script.threading.tasks;

/** Yog Script's {@code TaskAwaiter<TResult>}: what awaits a {@link Task1}, its result. */
public final class TaskAwaiter1<TResult> extends TaskAwaiter {
    TaskAwaiter1(Task1<TResult> task) {
        super(task);
    }

    /** Its {@code new GetResult()} (named after its type: it hides {@code TaskAwaiter}'s). */
    @SuppressWarnings("unchecked")
    public TResult TaskAwaiter$GetResult() {
        return ((Task1<TResult>) task).Result();
    }
}
