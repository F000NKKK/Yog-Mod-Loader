package dev.yog.script.threading.tasks;

import dev.yog.script.Action;
import dev.yog.script.threading.SynchronizationContext;
import dev.yog.script.threading.ThreadPool;

/**
 * Yog Script's {@code System.Threading.Tasks.TaskScheduler}: where tasks a
 * {@link TaskFactory} starts run. {@link #Default()} is the pool's; one of
 * a synchronization context posts to it (a side's thread, a UI's). A mod
 * writes its own by overriding {@link #QueueTask}.
 */
public abstract class TaskScheduler {
    private static final TaskScheduler POOL = new TaskScheduler() {
        @Override
        public void QueueTask(Action work) {
            ThreadPool.QueueUserWorkItem(work);
        }
    };

    /** The scheduler the running task was started by, while it runs. */
    private static final java.lang.ThreadLocal<TaskScheduler> RUNNING = new java.lang.ThreadLocal<>();

    protected TaskScheduler() {}

    /** Runs {@code work} where this scheduler runs tasks. */
    public abstract void QueueTask(Action work);

    public static TaskScheduler Default() {
        return POOL;
    }

    /** The scheduler of the task running here; the pool's outside one. */
    public static TaskScheduler Current() {
        TaskScheduler running = RUNNING.get();
        return running == null ? POOL : running;
    }

    /** A scheduler posting to the thread asking's synchronization context. */
    public static TaskScheduler FromCurrentSynchronizationContext() {
        SynchronizationContext context = SynchronizationContext.Current();
        if (context == null) {
            throw new dev.yog.script.YsPanic("this thread has no synchronization context", "TaskScheduler.FromCurrentSynchronizationContext");
        }
        return new TaskScheduler() {
            @Override
            public void QueueTask(Action work) {
                context.Post(work);
            }
        };
    }

    /** {@code work} run by this scheduler, {@link #Current()} it meanwhile. */
    void run(Runnable given) {
        Runnable work = dev.yog.script.threading.ExecutionContext.flowing(given);
        QueueTask(() -> {
            TaskScheduler outer = RUNNING.get();
            RUNNING.set(this);
            try {
                work.run();
            } finally {
                RUNNING.set(outer);
            }
        });
    }
}
