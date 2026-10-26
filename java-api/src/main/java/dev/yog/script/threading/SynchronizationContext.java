package dev.yog.script.threading;

import dev.yog.script.Action;

/**
 * Yog Script's {@code System.Threading.SynchronizationContext}, as .NET's:
 * where work handed to a thread runs. Each side's game thread has its own
 * (its ticks run what's posted); a thread may set one of its own (a UI
 * loop's). {@code await} goes on in the context it started in. The base
 * class posts to the pool and sends in place.
 */
public class SynchronizationContext {
    private static final java.lang.ThreadLocal<SynchronizationContext> SET = new java.lang.ThreadLocal<>();

    public SynchronizationContext() {}

    /** The context of the thread asking: its own set, else its side's; none on another. */
    public static SynchronizationContext Current() {
        SynchronizationContext set = SET.get();
        if (set != null) {
            return set;
        }
        dev.yog.script.threading.tasks.Scheduler side = dev.yog.script.threading.tasks.Scheduler.current();
        return side == null ? null : side.context();
    }

    /** {@code context} this thread's from now on ({@code null}: its side's again). */
    public static void SetSynchronizationContext(SynchronizationContext context) {
        SET.set(context);
    }

    /** {@code work} run in the context, later. */
    public void Post(Action work) {
        ThreadPool.QueueUserWorkItem(work);
    }

    /** {@code work} run in the context, waited for. */
    public void Send(Action work) {
        work.Invoke();
    }

    /** Whether the thread asking is the one this context runs work on. */
    public boolean IsCurrent() {
        return Current() == this;
    }
}
