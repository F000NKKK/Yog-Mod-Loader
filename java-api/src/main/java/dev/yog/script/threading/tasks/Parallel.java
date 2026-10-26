package dev.yog.script.threading.tasks;

import dev.yog.script.Action1;
import dev.yog.script.threading.ThreadPool;
import java.util.ArrayList;
import java.util.List;

/**
 * Yog Script's {@code System.Threading.Tasks.Parallel}: work split over
 * the pool and waited for — in a few parts per pool thread, not a task per
 * item, so a long range costs a handful of tasks.
 */
public final class Parallel {
    private Parallel() {}

    /** Parts per pool thread: enough for uneven items to even out. */
    private static final int PARTS_PER_THREAD = 4;

    public static void For(int fromInclusive, int toExclusive, Action1<Integer> body) {
        int count = toExclusive - fromInclusive;
        if (count <= 0) {
            return;
        }
        int parts = Math.min(count, ThreadPool.ThreadCount() * PARTS_PER_THREAD);
        int size = (count + parts - 1) / parts;
        List<Task> running = new ArrayList<>();
        for (int start = fromInclusive; start < toExclusive; start += size) {
            int from = start;
            int to = Math.min(toExclusive, start + size);
            running.add(Task.Run(() -> {
                for (int i = from; i < to; i++) {
                    body.Invoke(i);
                }
            }));
        }
        Task.WaitAll(running.toArray(new Task[0]));
    }

    public static <T> void ForEach(Iterable<T> items, Action1<T> body) {
        List<T> all = new ArrayList<>();
        items.forEach(all::add);
        For(0, all.size(), i -> body.Invoke(all.get(i)));
    }
}
