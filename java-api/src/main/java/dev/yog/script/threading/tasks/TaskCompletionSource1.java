package dev.yog.script.threading.tasks;

/** Yog Script's {@code TaskCompletionSource<TResult>}: a task given its result by hand. */
public final class TaskCompletionSource1<TResult> {
    private final Task1<TResult> task = new Task1<>();

    public Task1<TResult> Task() {
        return task;
    }

    public void SetResult(TResult result) {
        if (!TrySetResult(result)) {
            throw Completions.settledAlready();
        }
    }

    public boolean TrySetResult(TResult result) {
        return task.tryFinish(result);
    }

    public void SetException(dev.yog.script.Error error) {
        if (!TrySetException(error)) {
            throw Completions.settledAlready();
        }
    }

    public boolean TrySetException(dev.yog.script.Error error) {
        return task.tryFail(Completions.failure(error));
    }
}
