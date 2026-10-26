package dev.yog.script.collections.generic;

/** A place in a {@link LinkedList}: its item, and its neighbours. */
public final class LinkedListNode<T> {
    private T value;
    LinkedList<T> list;
    LinkedListNode<T> next;
    LinkedListNode<T> previous;

    public LinkedListNode(T value) {
        this.value = value;
    }

    public T Value() {
        return value;
    }

    public void set_Value(T value) {
        this.value = value;
    }

    public LinkedList<T> List() {
        return list;
    }

    public LinkedListNode<T> Next() {
        return next;
    }

    public LinkedListNode<T> Previous() {
        return previous;
    }
}
