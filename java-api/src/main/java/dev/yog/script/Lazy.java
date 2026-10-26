package dev.yog.script;

/** A value made the first time it's asked for, then kept: Yog Script's {@code Lazy<T>} (made once, whoever asks). */
public class Lazy<T> {
    private final Func1<T> valueFactory;
    private T value;
    private volatile boolean created;

    public Lazy(Func1<T> valueFactory) {
        this.valueFactory = valueFactory;
    }

    public T Value() {
        if (!created) {
            synchronized (this) {
                if (!created) {
                    value = valueFactory.Invoke();
                    created = true;
                }
            }
        }
        return value;
    }

    public boolean IsValueCreated() {
        return created;
    }
}
