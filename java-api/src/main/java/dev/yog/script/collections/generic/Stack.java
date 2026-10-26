package dev.yog.script.collections.generic;

import java.util.ArrayDeque;
import java.util.Iterator;

/** Items last in, first out: Yog Script's {@code Stack<T>}, gone through from its top. */
public class Stack<T> implements IReadOnlyCollection<T>, dev.yog.script.Typed {
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

    private static final String EMPTY = "the stack is empty";

    private final ArrayDeque<T> items = new ArrayDeque<>();

    public Stack() {}

    public Stack(IEnumerable<T> collection) {
        for (T item : collection) {
            items.addFirst(item);
        }
    }

    @Override
    public Iterator<T> iterator() {
        return items.iterator();
    }

    @Override
    public IEnumerator<T> GetEnumerator() {
        return Enumerators.of(items::iterator);
    }

    @Override
    public int Count() {
        return items.size();
    }

    public void Push(T item) {
        items.addFirst(item);
    }

    public T Pop() {
        if (items.isEmpty()) {
            throw new IllegalStateException(EMPTY);
        }
        return items.removeFirst();
    }

    public T Peek() {
        if (items.isEmpty()) {
            throw new IllegalStateException(EMPTY);
        }
        return items.getFirst();
    }

    public T TryPop() {
        return items.pollFirst();
    }

    public T TryPeek() {
        return items.peekFirst();
    }

    public void Clear() {
        items.clear();
    }

    public boolean Contains(T item) {
        return items.contains(item);
    }

    /** Its items in an array of objects (made the array wanted where it's handed on). */
    public Object[] ToArray() {
        return items.toArray();
    }
}
