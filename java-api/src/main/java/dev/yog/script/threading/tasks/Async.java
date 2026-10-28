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
    /** What a hot reload made of it (its machine as the new classes have it): what goes on instead. */
    private volatile Async<?> movedTo;

    /** The machines waiting mid-way; weak: a machine whose task is gone goes. */
    private static final java.util.Set<Async<?>> WAITING =
            java.util.Collections.synchronizedSet(java.util.Collections.newSetFromMap(new java.util.WeakHashMap<>()));

    /**
     * Each waiting machine {@code of} a reloaded generation carried into its
     * new class by {@code remake} (its state, locals and task kept): when
     * what it waits for is done, the new one goes on from where it was.
     */
    public static void carry(java.util.function.Predicate<Async<?>> of, java.util.function.Function<Object, Object> remake) {
        java.util.List<Async<?>> old;
        synchronized (WAITING) {
            old = WAITING.stream().filter(of).toList();
        }
        for (Async<?> m : old) {
            if (remake.apply(m) instanceof Async<?> next) {
                m.movedTo = next;
                WAITING.remove(m);
                WAITING.add(next);
            }
        }
    }

    /** The method's code, from {@link #$state} on. */
    protected abstract void $step();

    @Override
    public final void run() {
        Async<?> next = movedTo;
        if (next != null) {
            next.run();
            return;
        }
        WAITING.remove(this);
        // What it changes flows on in it, not back to whoever ran it.
        java.util.Map<Object, Object> outer = dev.yog.script.threading.ExecutionContext.capture();
        try {
            $step();
        } catch (RuntimeException e) {
            if (logged) {
                YogLog.error("Yog Script: an async method panicked: " + e.getMessage());
            }
            task.fail(e);
        } finally {
            dev.yog.script.threading.ExecutionContext.restore(outer);
        }
    }

    /** Whether it waits for {@code awaited}: not done yet, it goes on when it is. */
    protected final boolean $suspend(Task awaited) {
        if (awaited.IsCompleted()) {
            return false;
        }
        WAITING.add(this);
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
