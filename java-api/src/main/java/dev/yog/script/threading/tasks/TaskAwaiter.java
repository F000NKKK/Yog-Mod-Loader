package dev.yog.script.threading.tasks;

import dev.yog.script.Action;

/** Yog Script's {@code TaskAwaiter}: what awaits a {@link Task}. */
public class TaskAwaiter {
    final Task task;

    TaskAwaiter(Task task) {
        this.task = task;
    }

    public boolean IsCompleted() {
        return task.IsCompleted();
    }

    /** {@code continuation} run once the task is done, in the awaiting context. */
    public void OnCompleted(Action continuation) {
        task.onDone(continuation::Invoke);
    }

    /** Waits for the task; its failure thrown. */
    public void GetResult() {
        task.Wait();
    }
}
