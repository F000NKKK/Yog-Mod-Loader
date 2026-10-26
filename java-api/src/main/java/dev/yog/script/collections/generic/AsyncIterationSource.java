package dev.yog.script.collections.generic;

import dev.yog.script.Func1;

/** An {@code async} iterator method's sequence: a new machine for each walk. */
public final class AsyncIterationSource<T> implements IAsyncEnumerable<T> {
    private final Func1<IAsyncEnumerator<T>> walk;

    public AsyncIterationSource(Func1<IAsyncEnumerator<T>> walk) {
        this.walk = walk;
    }

    @Override
    public IAsyncEnumerator<T> GetAsyncEnumerator() {
        return walk.Invoke();
    }
}
