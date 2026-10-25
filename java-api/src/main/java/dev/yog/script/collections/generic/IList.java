package dev.yog.script.collections.generic;

/** Items in order, read and written by their index. */
public interface IList<T> extends ICollection<T> {
    T get_Item(int index);

    void set_Item(int index, T value);

    int IndexOf(T item);

    void Insert(int index, T item);

    void RemoveAt(int index);
}
