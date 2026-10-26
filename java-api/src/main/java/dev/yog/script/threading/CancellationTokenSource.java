package dev.yog.script.threading;

import dev.yog.script.Action;
import dev.yog.script.IDisposable;
import dev.yog.script.TimeSpan;
import java.util.List;
import java.util.concurrent.CopyOnWriteArrayList;
import java.util.concurrent.Executors;
import java.util.concurrent.ScheduledExecutorService;
import java.util.concurrent.TimeUnit;

/** Yog Script's {@code System.Threading.CancellationTokenSource}: what cancels. */
public final class CancellationTokenSource implements IDisposable {
    /** What cancels a source after a delay: one daemon thread for every source. */
    private static final ScheduledExecutorService LATER = Executors.newSingleThreadScheduledExecutor(r -> {
        java.lang.Thread t = new java.lang.Thread(r, "yog-cancel-after");
        t.setDaemon(true);
        return t;
    });

    private volatile boolean cancelled;
    private final List<Action> callbacks = new CopyOnWriteArrayList<>();
    private final CancellationToken token = new CancellationToken(this);

    public CancellationTokenSource() {}

    public CancellationToken Token() {
        return token;
    }

    public boolean IsCancellationRequested() {
        return cancelled;
    }

    public void Cancel() {
        if (cancelled) {
            return;
        }
        cancelled = true;
        for (Action a : callbacks) {
            a.Invoke();
        }
        callbacks.clear();
    }

    public void CancelAfter(TimeSpan delay) {
        LATER.schedule(this::Cancel, Thread.millis(delay), TimeUnit.MILLISECONDS);
    }

    public static CancellationTokenSource Linked(CancellationToken a, CancellationToken b) {
        CancellationTokenSource linked = new CancellationTokenSource();
        a.Register(linked::Cancel);
        b.Register(linked::Cancel);
        return linked;
    }

    @Override
    public void Dispose() {
        callbacks.clear();
    }

    /** {@code callback} run when it's cancelled (now if it is): what takes it back. */
    IDisposable register(Action callback) {
        if (cancelled) {
            callback.Invoke();
            return () -> {};
        }
        callbacks.add(callback);
        if (cancelled && callbacks.remove(callback)) {
            callback.Invoke();
        }
        return () -> callbacks.remove(callback);
    }
}
