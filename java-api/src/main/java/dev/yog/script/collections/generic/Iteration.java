package dev.yog.script.collections.generic;

import dev.yog.script.YsPanic;

/**
 * An iterator as the compiler writes it: a machine of states (the
 * {@code async} one's), each {@code MoveNext} running it from its
 * {@link #$state} to its next {@code yield return} ({@link #$yield}) or
 * its end ({@link #$finish}).
 */
public abstract class Iteration<T> implements IEnumerator<T> {
    /** Where {@link #$step} goes on from. */
    protected int $state;
    private T current;
    private boolean produced;
    private boolean done;

    /** The iterator's code, from {@link #$state} on. */
    protected abstract void $step();

    /** {@code yield return item}: the next item. */
    protected final void $yield(T item) {
        current = item;
        produced = true;
    }

    /** Its end ({@code yield break}, or its code run out). */
    protected final void $finish(Object nothing) {
        done = true;
    }

    @Override
    public final boolean MoveNext() {
        if (done) {
            return false;
        }
        produced = false;
        $step();
        return produced;
    }

    @Override
    public final T Current() {
        return current;
    }

    /** As C#'s iterators: a walk isn't started over, a new one is asked for. */
    @Override
    public final void Reset() {
        throw new YsPanic("an iterator isn't reset: walk its sequence again", "Reset");
    }
}
