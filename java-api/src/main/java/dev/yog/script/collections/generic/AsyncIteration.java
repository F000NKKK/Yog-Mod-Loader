package dev.yog.script.collections.generic;

import dev.yog.script.threading.tasks.Task;
import dev.yog.script.threading.tasks.Task1;

/**
 * An {@code async} iterator as the compiler writes it: the {@code async}
 * machine whose {@code yield return} ({@link #$yield}) finishes the
 * pending {@code MoveNextAsync} with {@code true}, its end
 * ({@link #$finish}) with {@code false}; an {@code await} in between
 * leaves it until what it awaits is done ({@link #$suspend}).
 */
public abstract class AsyncIteration<T> implements IAsyncEnumerator<T>, Runnable {
    /** Where {@link #$step} goes on from. */
    protected int $state;
    private T current;
    private boolean done;
    private Task1<Boolean> pending;
    /** What the step being run gives the pending ask (its item or its end), given once it's left. */
    private Boolean given;

    /** The iterator's code, from {@link #$state} on. */
    protected abstract void $step();

    @Override
    public final Task1<Boolean> MoveNextAsync() {
        pending = new Task1<>();
        if (done) {
            pending.finish(false);
        } else {
            run();
        }
        return pending;
    }

    @Override
    public final void run() {
        java.util.Map<Object, Object> outer = dev.yog.script.threading.ExecutionContext.capture();
        try {
            $step();
        } catch (RuntimeException e) {
            done = true;
            given = null;
            pending.fail(e);
        } finally {
            dev.yog.script.threading.ExecutionContext.restore(outer);
        }
        // Given after the step's left: what goes on then (the next ask)
        // never runs it again from the middle.
        Boolean answer = given;
        if (answer != null) {
            given = null;
            pending.finish(answer);
        }
    }

    @Override
    public final T Current() {
        return current;
    }

    /** {@code yield return item}. */
    protected final void $yield(T item) {
        current = item;
        given = true;
    }

    /** Its end. */
    protected final void $finish(Object nothing) {
        done = true;
        given = false;
    }

    /** Whether it waits for {@code awaited}: not done yet, it goes on when it is. */
    protected final boolean $suspend(Task awaited) {
        if (awaited.IsCompleted()) {
            return false;
        }
        awaited.onDone(this);
        return true;
    }
}
