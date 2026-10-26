package dev.yog.script.threading;

import dev.yog.script.TimeSpan;
import dev.yog.script.threading.tasks.Task;
import java.util.ArrayDeque;
import java.util.ArrayList;
import java.util.List;

/**
 * Yog Script's {@code System.Threading.Signal}: a flag waited for ({@code
 * ManualResetEventSlim}); auto-reset, one waiter let through by each set
 * ({@code AutoResetEvent}).
 */
public final class Signal {
    private final boolean autoReset;
    private boolean set;
    private final ArrayDeque<Task> waiting = new ArrayDeque<>();

    public Signal() {
        this(false, false);
    }

    public Signal(boolean initialState) {
        this(initialState, false);
    }

    public Signal(boolean initialState, boolean autoReset) {
        this.set = initialState;
        this.autoReset = autoReset;
    }

    public void Set() {
        List<Task> woken = new ArrayList<>();
        synchronized (this) {
            if (autoReset) {
                Task one = waiting.pollFirst();
                if (one == null) {
                    set = true;
                } else {
                    woken.add(one);
                }
            } else {
                set = true;
                woken.addAll(waiting);
                waiting.clear();
            }
        }
        woken.forEach(Task::finish);
    }

    public synchronized void Reset() {
        set = false;
    }

    public Task WaitAsync() {
        synchronized (this) {
            if (set) {
                if (autoReset) {
                    set = false;
                }
                return Task.CompletedTask();
            }
            Task t = new Task();
            waiting.addLast(t);
            return t;
        }
    }

    public void Wait() {
        WaitAsync().Wait();
    }

    public boolean Wait(TimeSpan timeout) {
        Task t = WaitAsync();
        if (t.Wait(timeout)) {
            return true;
        }
        synchronized (this) {
            return !waiting.remove(t);
        }
    }

    public synchronized boolean IsSet() {
        return set;
    }
}
