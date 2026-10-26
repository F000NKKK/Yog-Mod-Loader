package dev.yog.script.threading.tasks;

import dev.yog.YogLog;

/**
 * An {@code async} method as the compiler writes it: a machine of states
 * ({@link #$state}), its locals that live across an {@code await} its
 * fields. {@link #$step} runs from its state on until it awaits a task not
 * done ({@link #$suspend}: it goes on when that one is) or finishes.
 */
public abstract class Async<T> implements Runnable {
    /** Where {@link #$step} goes on from. */
    protected int $state;
    private final Task1<T> task = new Task1<>();
    /** An {@code async void}'s: nobody awaits it, so its panic is logged. */
    private boolean logged;

    /** The method's code, from {@link #$state} on. */
    protected abstract void $step();

    @Override
    public final void run() {
        try {
            $step();
        } catch (RuntimeException e) {
            if (logged) {
                YogLog.error("Yog Script: an async method panicked: " + e.getMessage());
            }
            task.fail(e);
        }
    }

    /** Whether it waits for {@code awaited}: not done yet, it goes on when it is. */
    protected final boolean $suspend(Task awaited) {
        if (awaited.IsCompleted()) {
            return false;
        }
        awaited.onDone(this);
        return true;
    }

    /** It finished, giving {@code value}. */
    protected final void $finish(T value) {
        task.finish(value);
    }

    /** What it gives: its task. */
    public final Task1<T> $task() {
        return task;
    }

    /** Started as an {@code async void}: what it gives isn't awaited. */
    public final void $start() {
        logged = true;
        run();
    }
}
