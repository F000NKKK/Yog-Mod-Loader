package dev.yog.script.collections.generic;

/** Items that can be counted. */
public interface IReadOnlyCollection<T> extends IEnumerable<T> {
    int Count();
}
