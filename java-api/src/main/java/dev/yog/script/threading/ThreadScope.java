package dev.yog.script.threading;

import dev.yog.script.Action;
import java.util.ArrayList;
import java.util.List;

/** Yog Script's {@code System.Threading.ThreadScope}: threads joined at the scope's end. */
public final class ThreadScope {
    private final List<ThreadHandle<Object>> started = new ArrayList<>();

    ThreadScope() {}

    public synchronized void Run(Action work) {
        started.add(ThreadHandle.started(() -> {
            work.Invoke();
            return null;
        }));
    }

    /** Each thread joined; the first panic among them thrown again. */
    void joinAll() {
        RuntimeException first = null;
        List<ThreadHandle<Object>> all;
        synchronized (this) {
            all = new ArrayList<>(started);
        }
        for (ThreadHandle<Object> h : all) {
            try {
                h.Join();
            } catch (RuntimeException e) {
                if (first == null) first = e;
            }
        }
        if (first != null) {
            throw first;
        }
    }
}
