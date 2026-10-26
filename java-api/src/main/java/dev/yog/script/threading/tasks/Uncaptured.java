package dev.yog.script.threading.tasks;

/**
 * A task awaited without its synchronization context
 * ({@code ConfigureAwait(false)}): what awaits it goes on where it's
 * finished. Everything else is the task's own.
 */
final class Uncaptured<TResult> extends Task1<TResult> {
    private final Task of;
    /** The task again when it gives a result. */
    private final Task1<TResult> giving;

    Uncaptured(Task of) {
        this.of = of;
        this.giving = null;
    }

    Uncaptured(Task1<TResult> of) {
        this.of = of;
        this.giving = of;
    }

    @Override
    public boolean IsCompleted() {
        return of.IsCompleted();
    }

    @Override
    public boolean IsCompletedSuccessfully() {
        return of.IsCompletedSuccessfully();
    }

    @Override
    public boolean IsFaulted() {
        return of.IsFaulted();
    }

    @Override
    public void onDone(Runnable then) {
        of.onDoneUncaptured(then);
    }

    @Override
    public void onDoneUncaptured(Runnable then) {
        of.onDoneUncaptured(then);
    }

    @Override
    public Throwable failureOf() {
        return of.failureOf();
    }

    @Override
    public void check() {
        of.check();
    }

    @Override
    public TResult Result() {
        return giving == null ? null : giving.Result();
    }
}
