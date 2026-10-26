package dev.yog.script.threading.tasks;

/** Yog Script's {@code Task<TResult>}: a task that finishes with a result. */
public class Task1<TResult> extends Task {
    private volatile TResult result;

    /** Its result, waited for as {@link #Wait()} does (on a side's thread, pumping it). */
    public TResult Result() {
        waitDone();
        check();
        return result;
    }

    /** Its {@code new GetAwaiter()} (named after its type: it hides {@code Task}'s). */
    public TaskAwaiter1<TResult> Task$GetAwaiter() {
        return new TaskAwaiter1<>(this);
    }

    /** Its {@code new ConfigureAwait} (named after its type: it hides {@code Task}'s). */
    public Task1<TResult> Task$ConfigureAwait(boolean continueOnCapturedContext) {
        return continueOnCapturedContext ? this : new Uncaptured<>(this);
    }

    /** It done with {@code value}. */
    public void finish(TResult value) {
        settle(DONE, null, () -> result = value);
    }
}
