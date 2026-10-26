package dev.yog.script.linq;

import dev.yog.script.collections.generic.IEnumerable;

/** The items of one key ({@code GroupBy}). */
public interface IGrouping<K, T> extends IEnumerable<T> {
    K Key();
}
