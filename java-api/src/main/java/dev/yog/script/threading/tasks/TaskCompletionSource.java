package dev.yog.script.threading.tasks;

/** Yog Script's {@code TaskCompletionSource}: a task settled by hand. */
public final class TaskCompletionSource {
    private final Task task = new Task();

    public Task Task() {
        return task;
    }

    public void SetResult() {
        if (!TrySetResult()) {
            throw Completions.settledAlready();
        }
    }

    public boolean TrySetResult() {
        return task.tryFinish();
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
