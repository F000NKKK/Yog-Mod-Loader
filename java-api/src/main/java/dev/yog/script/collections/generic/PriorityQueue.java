package dev.yog.script.collections.generic;

import java.util.Comparator;

/** Items taken least priority first: Yog Script's {@code PriorityQueue<TElement, TPriority>}. */
public class PriorityQueue<E, P> implements dev.yog.script.Typed {
    /** The type arguments it was made with ({@code GetType()}'s), when they were known. */
    private dev.yog.script.Type[] $types;

    @Override
    public dev.yog.script.Type[] $types() {
        return $types;
    }

    @Override
    public void $types(dev.yog.script.Type[] arguments) {
        this.$types = arguments;
    }

    private static final String EMPTY = "the queue is empty";

    /** An item and its priority. */
    private record Entry<E, P>(E element, P priority) {}

    private final Comparator<P> order;
    private final java.util.PriorityQueue<Entry<E, P>> items;

    public PriorityQueue() {
        this(Order.natural());
    }

    public PriorityQueue(IComparer<P> comparer) {
        this(comparer.comparator());
    }

    private PriorityQueue(Comparator<P> order) {
        this.order = order;
        this.items = new java.util.PriorityQueue<>(Comparator.comparing(Entry::priority, order));
    }

    public int Count() {
        return items.size();
    }

    public void Enqueue(E element, P priority) {
        items.add(new Entry<>(element, priority));
    }

    public E Dequeue() {
        Entry<E, P> least = items.poll();
        if (least == null) {
            throw new IllegalStateException(EMPTY);
        }
        return least.element();
    }

    public E Peek() {
        Entry<E, P> least = items.peek();
        if (least == null) {
            throw new IllegalStateException(EMPTY);
        }
        return least.element();
    }

    public E TryDequeue() {
        Entry<E, P> least = items.poll();
        return least == null ? null : least.element();
    }

    public E TryPeek() {
        Entry<E, P> least = items.peek();
        return least == null ? null : least.element();
    }

    public E EnqueueDequeue(E element, P priority) {
        Entry<E, P> least = items.peek();
        if (least == null || order.compare(priority, least.priority()) <= 0) {
            return element;
        }
        items.poll();
        items.add(new Entry<>(element, priority));
        return least.element();
    }

    public void Clear() {
        items.clear();
    }
}
