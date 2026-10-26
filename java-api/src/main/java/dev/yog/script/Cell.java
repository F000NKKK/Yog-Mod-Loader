package dev.yog.script;

/** Yog Script's {@code System.Cell<T>}: a value changed through a shared reference, on one thread. */
public final class Cell<T> extends TypedObject {
    private T value;

    public Cell(T value) {
        this.value = value;
    }

    public T Get() {
        return value;
    }

    public void Set(T value) {
        this.value = value;
    }

    public T Replace(T value) {
        T old = this.value;
        this.value = value;
        return old;
    }

    public void Update(Func2<T, T> change) {
        value = change.Invoke(value);
    }
}
