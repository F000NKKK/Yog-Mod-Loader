package dev.yog.script;

/** A value made the first time it's asked for, then kept: Yog Script's {@code Lazy<T>} (made once, whoever asks). */
public class Lazy<T> implements dev.yog.script.Typed {
    /** The type arguments it was made with ({@code GetType()}'s), when they were known. */
    private dev.yog.script.Type[] $types;

    @Override
    public dev.yog.script.Type[] $types() {
        return $types;
    }

    @Override
    public void $types(dev.yog.script.Type[] arguments) {
        this.$types = arguments;
    }

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
