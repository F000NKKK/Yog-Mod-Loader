package dev.yog.script.collections.generic;

import java.util.Iterator;
import java.util.NoSuchElementException;

/** Items linked each to the next and the one before: Yog Script's {@code LinkedList<T>}. */
public class LinkedList<T> implements ICollection<T>, IReadOnlyCollection<T>, dev.yog.script.Typed {
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

    private static final String EMPTY = "the list is empty";
    private static final String ELSEWHERE = "the node isn't this list's";

    private LinkedListNode<T> first;
    private LinkedListNode<T> last;
    private int count;

    public LinkedList() {}

    public LinkedList(IEnumerable<T> collection) {
        for (T item : collection) {
            link(new LinkedListNode<>(item), last, null);
        }
    }

    @Override
    public Iterator<T> iterator() {
        return new Iterator<>() {
            private LinkedListNode<T> at = first;

            @Override
            public boolean hasNext() {
                return at != null;
            }

            @Override
            public T next() {
                if (at == null) {
                    throw new NoSuchElementException();
                }
                T value = at.Value();
                at = at.next;
                return value;
            }
        };
    }

    @Override
    public IEnumerator<T> GetEnumerator() {
        return Enumerators.of(this::iterator);
    }

    @Override
    public int Count() {
        return count;
    }

    @Override
    public boolean IsReadOnly() {
        return false;
    }

    public LinkedListNode<T> First() {
        return first;
    }

    public LinkedListNode<T> Last() {
        return last;
    }

    public LinkedListNode<T> AddFirst(T value) {
        return link(new LinkedListNode<>(value), null, first);
    }

    public LinkedListNode<T> AddLast(T value) {
        return link(new LinkedListNode<>(value), last, null);
    }

    public LinkedListNode<T> AddBefore(LinkedListNode<T> node, T value) {
        mine(node);
        return link(new LinkedListNode<>(value), node.previous, node);
    }

    public LinkedListNode<T> AddAfter(LinkedListNode<T> node, T value) {
        mine(node);
        return link(new LinkedListNode<>(value), node, node.next);
    }

    /** {@code ICollection<T>.Add}, explicitly: at its end. */
    @Override
    public void Add(T item) {
        AddLast(item);
    }

    public LinkedListNode<T> Find(T value) {
        for (LinkedListNode<T> at = first; at != null; at = at.next) {
            if (java.util.Objects.equals(at.Value(), value)) {
                return at;
            }
        }
        return null;
    }

    public LinkedListNode<T> FindLast(T value) {
        for (LinkedListNode<T> at = last; at != null; at = at.previous) {
            if (java.util.Objects.equals(at.Value(), value)) {
                return at;
            }
        }
        return null;
    }

    @Override
    public boolean Remove(T value) {
        LinkedListNode<T> node = Find(value);
        if (node == null) {
            return false;
        }
        unlink(node);
        return true;
    }

    public void Remove(LinkedListNode<T> node) {
        mine(node);
        unlink(node);
    }

    public void RemoveFirst() {
        if (first == null) {
            throw new IllegalStateException(EMPTY);
        }
        unlink(first);
    }

    public void RemoveLast() {
        if (last == null) {
            throw new IllegalStateException(EMPTY);
        }
        unlink(last);
    }

    @Override
    public void Clear() {
        while (first != null) {
            unlink(first);
        }
    }

    @Override
    public boolean Contains(T value) {
        return Find(value) != null;
    }

    /** {@code node} put between {@code before} and {@code after}. */
    private LinkedListNode<T> link(LinkedListNode<T> node, LinkedListNode<T> before, LinkedListNode<T> after) {
        node.list = this;
        node.previous = before;
        node.next = after;
        if (before == null) {
            first = node;
        } else {
            before.next = node;
        }
        if (after == null) {
            last = node;
        } else {
            after.previous = node;
        }
        count++;
        return node;
    }

    private void unlink(LinkedListNode<T> node) {
        if (node.previous == null) {
            first = node.next;
        } else {
            node.previous.next = node.next;
        }
        if (node.next == null) {
            last = node.previous;
        } else {
            node.next.previous = node.previous;
        }
        node.list = null;
        node.next = null;
        node.previous = null;
        count--;
    }

    private void mine(LinkedListNode<T> node) {
        if (node.list != this) {
            throw new IllegalArgumentException(ELSEWHERE);
        }
    }
}
