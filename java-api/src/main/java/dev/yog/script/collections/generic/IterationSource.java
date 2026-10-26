package dev.yog.script.collections.generic;

import dev.yog.script.Func1;

/** An iterator method's sequence: a new machine for each walk, as C#'s. */
public final class IterationSource<T> implements IEnumerable<T> {
    private final Func1<IEnumerator<T>> walk;

    public IterationSource(Func1<IEnumerator<T>> walk) {
        this.walk = walk;
    }

    @Override
    public IEnumerator<T> GetEnumerator() {
        return walk.Invoke();
    }
}
