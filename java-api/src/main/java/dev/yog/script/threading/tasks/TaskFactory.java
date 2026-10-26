package dev.yog.script.threading.tasks;

import dev.yog.script.Action;
import dev.yog.script.Func1;

/** Yog Script's {@code System.Threading.Tasks.TaskFactory}: tasks started on a {@link TaskScheduler}. */
public final class TaskFactory {
    private final TaskScheduler scheduler;

    public TaskFactory() {
        this(TaskScheduler.Default());
    }

    public TaskFactory(TaskScheduler scheduler) {
        this.scheduler = scheduler;
    }

    public TaskScheduler Scheduler() {
        return scheduler;
    }

    public <TResult> Task1<TResult> StartNew(Func1<TResult> work) {
        Task1<TResult> t = new Task1<>();
        scheduler.run(() -> {
            try {
                t.finish(work.Invoke());
            } catch (RuntimeException e) {
                t.fail(e);
            }
        });
        return t;
    }

    public Task StartNew(Action work) {
        Task t = new Task();
        scheduler.run(() -> {
            try {
                work.Invoke();
                t.finish();
            } catch (RuntimeException e) {
                t.fail(e);
            }
        });
        return t;
    }
}
