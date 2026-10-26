package dev.yog.script;

/** Yog Script's {@code System.Box<T>}: a value on the heap with one owner. */
public final class Box<T> extends TypedObject {
    private T value;

    public Box(T value) {
        this.value = value;
    }

    public T Value() {
        return value;
    }

    public void set_Value(T value) {
        this.value = value;
    }
}
