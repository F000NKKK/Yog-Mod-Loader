package dev.yog.script.threading.channels;

import dev.yog.script.TypedObject;
import dev.yog.script.threading.tasks.Task;
import dev.yog.script.threading.tasks.Task1;
import java.util.ArrayDeque;
import java.util.ArrayList;
import java.util.List;

/**
 * Yog Script's {@code Channel<T>}: items in order, many writers, many
 * readers. Its state is under its own monitor; tasks waiting on it are
 * woken outside it (their continuations go back to their own contexts).
 */
public final class Channel1<T> extends TypedObject {
    private final int capacity;
    private final BoundedChannelFullMode fullMode;
    private final ArrayDeque<T> items = new ArrayDeque<>();
    /** Readers waiting for an item ({@code WaitToReadAsync}: true when one comes, false at the end). */
    private final List<Task1<Boolean>> readers = new ArrayList<>();
    /** Writers waiting for room: each its item and its task. */
    private final ArrayDeque<Pending<T>> writers = new ArrayDeque<>();
    private boolean completed;
    private final Task completion = new Task();
    private final ChannelWriter<T> writer = new ChannelWriter<>(this);
    private final ChannelReader<T> reader = new ChannelReader<>(this);

    Channel1(int capacity, BoundedChannelFullMode fullMode) {
        this.capacity = capacity;
        this.fullMode = fullMode;
    }

    public ChannelWriter<T> Writer() {
        return writer;
    }

    public ChannelReader<T> Reader() {
        return reader;
    }

    private record Pending<T>(T item, Task done) {}

    /** {@code item} written now, as the full mode says when there's no room: whether it went in. */
    boolean tryWrite(T item) {
        List<Task1<Boolean>> woken;
        synchronized (this) {
            if (completed) {
                return false;
            }
            if (items.size() >= capacity) {
                if (fullMode == BoundedChannelFullMode.DropWrite) {
                    return true;
                }
                if (fullMode == BoundedChannelFullMode.DropNewest) {
                    items.pollLast();
                } else if (fullMode == BoundedChannelFullMode.DropOldest) {
                    items.pollFirst();
                } else {
                    return false;
                }
            }
            items.addLast(item);
            woken = takeReaders();
        }
        woken.forEach(t -> t.finish(true));
        return true;
    }

    /** {@code item} written once there's room. */
    Task writeAsync(T item) {
        if (tryWrite(item)) {
            return Task.CompletedTask();
        }
        Task done = new Task();
        synchronized (this) {
            if (completed) {
                done.fail(new dev.yog.script.YsPanic("the channel is completed", "ChannelWriter.WriteAsync"));
                return done;
            }
            if (items.size() < capacity) {
                // Room came between.
                items.addLast(item);
                done.finish();
            } else {
                writers.addLast(new Pending<>(item, done));
            }
        }
        return done;
    }

    void complete() {
        List<Task1<Boolean>> woken;
        synchronized (this) {
            if (completed) {
                return;
            }
            completed = true;
            woken = takeReaders();
        }
        woken.forEach(t -> t.finish(!items.isEmpty()));
        finishIfDrained();
    }

    /** The next item, if there's one now (a waiting writer's moving in after it). */
    T tryRead() {
        T item;
        Pending<T> moved = null;
        synchronized (this) {
            item = items.pollFirst();
            if (item != null && !writers.isEmpty()) {
                moved = writers.pollFirst();
                items.addLast(moved.item());
            }
        }
        if (moved != null) {
            moved.done().finish();
        }
        finishIfDrained();
        return item;
    }

    /** Whether an item comes: {@code false} once completed and read. */
    Task1<Boolean> waitToRead() {
        Task1<Boolean> t = new Task1<>();
        boolean now;
        synchronized (this) {
            if (!items.isEmpty() || completed) {
                now = !items.isEmpty();
            } else {
                readers.add(t);
                return t;
            }
        }
        t.finish(now);
        return t;
    }

    synchronized int count() {
        return items.size();
    }

    Task completion() {
        return completion;
    }

    private List<Task1<Boolean>> takeReaders() {
        List<Task1<Boolean>> woken = new ArrayList<>(readers);
        readers.clear();
        return woken;
    }

    private void finishIfDrained() {
        boolean drained;
        synchronized (this) {
            drained = completed && items.isEmpty();
        }
        if (drained) {
            completion.finish();
        }
    }
}
