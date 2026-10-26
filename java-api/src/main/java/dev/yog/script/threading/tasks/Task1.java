package dev.yog.script.threading.tasks;

/** Yog Script's {@code Task<TResult>}: a task that finishes with a result. */
public class Task1<TResult> extends Task {
    private TResult result;

    public TResult Result() {
        if (!IsCompleted()) {
            throw new dev.yog.script.YsPanic("the task isn't done: await it", "");
        }
        check();
        return result;
    }

    /** It done with {@code value}. */
    public void finish(TResult value) {
        result = value;
        finish();
    }
}
