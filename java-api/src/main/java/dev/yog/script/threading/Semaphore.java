package dev.yog.script.threading;

import dev.yog.script.TimeSpan;
import dev.yog.script.YsPanic;
import dev.yog.script.threading.tasks.Task;
import java.util.ArrayDeque;
import java.util.ArrayList;
import java.util.List;

/**
 * Yog Script's {@code System.Threading.Semaphore} ({@code SemaphoreSlim}):
 * at most so many at once. Waiters, blocking or awaiting, enter in the
 * order they came; a blocking wait on a side's thread pumps it (as
 * {@code Task.Wait} does).
 */
public final class Semaphore {
    private final int max;
    private int count;
    private final ArrayDeque<Task> waiting = new ArrayDeque<>();

    public Semaphore(int initialCount) {
        this(initialCount, Integer.MAX_VALUE);
    }

    public Semaphore(int initialCount, int maxCount) {
        this.count = initialCount;
        this.max = maxCount;
    }

    public Task WaitAsync() {
        synchronized (this) {
            if (count > 0 && waiting.isEmpty()) {
                count--;
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

    /** As {@code Wait()}, at most {@code timeout}: whether it entered (a waiter that ran out leaves the line). */
    public boolean Wait(TimeSpan timeout) {
        Task t = WaitAsync();
        if (t.Wait(timeout)) {
            return true;
        }
        synchronized (this) {
            if (waiting.remove(t)) {
                return false;
            }
        }
        // Let in as it ran out.
        return true;
    }

    public int Release() {
        return Release(1);
    }

    public int Release(int releaseCount) {
        List<Task> woken = new ArrayList<>();
        int before;
        synchronized (this) {
            before = count;
            if (count + releaseCount > max) {
                throw new YsPanic("released past the semaphore's most", "Semaphore.Release");
            }
            int left = releaseCount;
            while (left > 0 && !waiting.isEmpty()) {
                woken.add(waiting.pollFirst());
                left--;
            }
            count += left;
        }
        woken.forEach(Task::finish);
        return before;
    }

    public synchronized int CurrentCount() {
        return count;
    }
}
