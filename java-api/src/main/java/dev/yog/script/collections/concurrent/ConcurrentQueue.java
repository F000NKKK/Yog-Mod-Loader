package dev.yog.script.collections.concurrent;

import dev.yog.script.TypedObject;
import java.util.concurrent.ConcurrentLinkedQueue;

/** Yog Script's {@code ConcurrentQueue<T>}: added to and taken from by many threads at once. */
public final class ConcurrentQueue<T> extends TypedObject {
    private final ConcurrentLinkedQueue<T> items = new ConcurrentLinkedQueue<>();

    public ConcurrentQueue() {}

    public void Enqueue(T item) {
        items.add(item);
    }

    public T TryDequeue() {
        return items.poll();
    }

    public T TryPeek() {
        return items.peek();
    }

    public int Count() {
        return items.size();
    }

    public boolean IsEmpty() {
        return items.isEmpty();
    }
}
