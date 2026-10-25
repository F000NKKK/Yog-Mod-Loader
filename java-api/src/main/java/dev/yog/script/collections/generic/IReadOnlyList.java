package dev.yog.script.collections.generic;

/** Items in order, read by their index. */
public interface IReadOnlyList<T> extends IReadOnlyCollection<T> {
    T get_Item(int index);
}
