package dev.yog.script.linq;

import dev.yog.script.collections.generic.IEnumerable;

/** Items in an order ({@code OrderBy}), to be ordered further ({@code ThenBy}). */
public interface IOrderedEnumerable<T> extends IEnumerable<T> {
    /** The order its items are in. */
    java.util.Comparator<T> order();

    /** The items it orders, as they came. */
    IEnumerable<T> source();
}
